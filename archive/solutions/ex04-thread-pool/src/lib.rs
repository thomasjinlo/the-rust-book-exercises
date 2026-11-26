//! Fixed-size thread pool with a bounded job queue.
//!
//! Queue: `Mutex<VecDeque<Job>>` plus `Condvar`s. No channels.

use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};

type Job = Box<dyn FnOnce() + Send + 'static>;

/// Errors from [`ThreadPool::new`] and [`ThreadPool::execute`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolError {
    /// `workers == 0`. Checked before capacity.
    ZeroWorkers,
    /// `queue_capacity == 0`.
    ZeroCapacity,
    /// The pool has begun shutting down and accepts no new jobs.
    Shutdown,
}

/// Errors from [`ThreadPool::try_execute`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TryExecuteError {
    /// The queue holds `queue_capacity` jobs.
    Full,
    /// The pool has begun shutting down and accepts no new jobs.
    Shutdown,
}

struct State {
    queue: VecDeque<Job>,
    closed: bool,
}

struct Shared {
    state: Mutex<State>,
    // Signalled when a job is pushed or the pool closes. Waited on by workers.
    not_empty: Condvar,
    // Signalled when a job is popped or the pool closes. Waited on by producers.
    not_full: Condvar,
    capacity: usize,
    panicked: AtomicUsize,
    completed: AtomicUsize,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        // Jobs run outside the lock, so poisoning can only come from a bug here.
        self.state.lock().expect("pool state poisoned")
    }
}

/// A pool of `workers` threads fed by a queue of at most `queue_capacity` jobs.
///
/// Jobs that are running do not count against the capacity.
/// The pool must be `Send + Sync` so it can be shared through `Arc`.
pub struct ThreadPool {
    shared: Arc<Shared>,
    workers: Vec<JoinHandle<()>>,
}

fn worker_loop(shared: &Shared) {
    loop {
        let job = {
            let mut st = shared.lock();
            loop {
                if let Some(job) = st.queue.pop_front() {
                    break job;
                }
                // Exit only on an empty queue: this is what drains on close.
                if st.closed {
                    return;
                }
                st = shared.not_empty.wait(st).expect("pool state poisoned");
            }
        };
        shared.not_full.notify_one();
        // A panic payload whose Drop panics again would kill the worker.
        match catch_unwind(AssertUnwindSafe(job)) {
            Ok(()) => shared.completed.fetch_add(1, Ordering::Relaxed),
            Err(_) => shared.panicked.fetch_add(1, Ordering::Relaxed),
        };
    }
}

impl ThreadPool {
    /// Starts `workers` threads. Fails with `ZeroWorkers`, then `ZeroCapacity`.
    ///
    /// # Panics
    /// If the OS refuses to spawn a thread.
    pub fn new(workers: usize, queue_capacity: usize) -> Result<ThreadPool, PoolError> {
        if workers == 0 {
            return Err(PoolError::ZeroWorkers);
        }
        if queue_capacity == 0 {
            return Err(PoolError::ZeroCapacity);
        }
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                queue: VecDeque::with_capacity(queue_capacity),
                closed: false,
            }),
            not_empty: Condvar::new(),
            not_full: Condvar::new(),
            capacity: queue_capacity,
            panicked: AtomicUsize::new(0),
            completed: AtomicUsize::new(0),
        });
        let handles = (0..workers)
            .map(|i| {
                let shared = Arc::clone(&shared);
                thread::Builder::new()
                    .name(format!("pool-worker-{i}"))
                    .spawn(move || worker_loop(&shared))
                    .expect("failed to spawn pool worker")
            })
            .collect();
        Ok(ThreadPool {
            shared,
            workers: handles,
        })
    }

    /// Queues `f`. Blocks while the queue is full.
    ///
    /// Returns `Err(PoolError::Shutdown)` if shutdown has begun, including when
    /// it begins while this call is blocked. In that case `f` is dropped unrun.
    pub fn execute<F: FnOnce() + Send + 'static>(&self, f: F) -> Result<(), PoolError> {
        let mut st = self.shared.lock();
        loop {
            if st.closed {
                return Err(PoolError::Shutdown);
            }
            if st.queue.len() < self.shared.capacity {
                break;
            }
            st = self.shared.not_full.wait(st).expect("pool state poisoned");
        }
        st.queue.push_back(Box::new(f));
        drop(st);
        self.shared.not_empty.notify_one();
        Ok(())
    }

    /// Queues `f` without blocking.
    pub fn try_execute<F: FnOnce() + Send + 'static>(&self, f: F) -> Result<(), TryExecuteError> {
        let mut st = self.shared.lock();
        if st.closed {
            return Err(TryExecuteError::Shutdown);
        }
        if st.queue.len() >= self.shared.capacity {
            return Err(TryExecuteError::Full);
        }
        st.queue.push_back(Box::new(f));
        drop(st);
        self.shared.not_empty.notify_one();
        Ok(())
    }

    /// Begins shutdown without waiting for it to finish.
    ///
    /// After this call `execute` and `try_execute` return `Shutdown`, blocked
    /// `execute` calls wake and return `Shutdown`, and workers exit once the
    /// queue is empty. Idempotent. Does not join workers.
    pub fn close(&self) {
        self.shared.lock().closed = true;
        self.shared.not_empty.notify_all();
        self.shared.not_full.notify_all();
    }

    /// Number of jobs waiting in the queue (not counting running jobs).
    pub fn queued(&self) -> usize {
        self.shared.lock().queue.len()
    }

    /// Number of jobs that panicked.
    pub fn panicked_jobs(&self) -> usize {
        self.shared.panicked.load(Ordering::Relaxed)
    }

    /// Number of jobs that ran to completion without panicking.
    pub fn completed_jobs(&self) -> usize {
        self.shared.completed.load(Ordering::Relaxed)
    }

    /// Closes the pool, runs every job already queued, then joins all workers.
    pub fn shutdown(self) {
        drop(self);
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        self.close();
        let me = thread::current().id();
        for handle in self.workers.drain(..) {
            // The last `Arc<ThreadPool>` may be released by a job on a worker.
            // A thread cannot join itself, so that worker is left to exit alone.
            if handle.thread().id() != me {
                let _ = handle.join();
            }
        }
    }
}
