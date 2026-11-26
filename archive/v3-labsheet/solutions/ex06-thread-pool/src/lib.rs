//! Fixed-size thread pool with a bounded job queue.
//!
//! This design feeds workers through one `sync_channel` shared behind a mutex.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

type Job = Box<dyn FnOnce() + Send + 'static>;

/// Errors from [`ThreadPool::new`] and [`ThreadPool::execute`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolError {
    /// `workers == 0`. Checked before capacity.
    ZeroWorkers,
    /// `queue_capacity == 0`.
    ZeroCapacity,
    /// The pool has been closed and accepts no new jobs.
    Shutdown,
}

/// Errors from [`ThreadPool::try_execute`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TryExecuteError {
    /// The queue already holds `queue_capacity` jobs.
    Full,
    /// The pool has been closed and accepts no new jobs.
    Shutdown,
}

/// A pool of `workers` threads fed by a queue of at most `queue_capacity` jobs.
///
/// Jobs that are running do not count against the capacity.
/// The pool must be `Send + Sync` so it can be shared through `Arc`.
pub struct ThreadPool {
    // `None` once closed. Workers see `recv` fail when every sender is gone and
    // the queue is empty, which is what makes them drain before exiting.
    sender: Mutex<Option<SyncSender<Job>>>,
    workers: Mutex<Vec<JoinHandle<()>>>,
    panicked: Arc<AtomicUsize>,
}

fn worker_loop(receiver: &Mutex<Receiver<Job>>, panicked: &AtomicUsize) {
    loop {
        // The guard is a temporary: it is released before the job runs, so
        // other workers can receive while this one works.
        let job = receiver.lock().expect("receiver lock poisoned").recv();
        let Ok(job) = job else { return };
        if catch_unwind(AssertUnwindSafe(job)).is_err() {
            panicked.fetch_add(1, Ordering::Relaxed);
        }
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
        let (tx, rx) = sync_channel::<Job>(queue_capacity);
        let rx = Arc::new(Mutex::new(rx));
        let panicked = Arc::new(AtomicUsize::new(0));
        let handles = (0..workers)
            .map(|i| {
                let (rx, panicked) = (Arc::clone(&rx), Arc::clone(&panicked));
                thread::Builder::new()
                    .name(format!("pool-worker-{i}"))
                    .spawn(move || worker_loop(&rx, &panicked))
                    .expect("failed to spawn pool worker")
            })
            .collect();
        Ok(ThreadPool {
            sender: Mutex::new(Some(tx)),
            workers: Mutex::new(handles),
            panicked,
        })
    }

    // Clone out of the lock: a blocking `send` must not hold it, or `close`
    // and other producers would stall behind a full queue.
    fn sender(&self) -> Option<SyncSender<Job>> {
        self.sender.lock().expect("sender lock poisoned").clone()
    }

    /// Queues `f`. Blocks while the queue is full.
    ///
    /// Returns `Err(PoolError::Shutdown)` if the pool is closed. Every job for
    /// which this returns `Ok(())` runs exactly once.
    pub fn execute<F: FnOnce() + Send + 'static>(&self, f: F) -> Result<(), PoolError> {
        let tx = self.sender().ok_or(PoolError::Shutdown)?;
        tx.send(Box::new(f)).map_err(|_| PoolError::Shutdown)
    }

    /// Queues `f` without blocking.
    ///
    /// `Shutdown` takes precedence over `Full`.
    pub fn try_execute<F: FnOnce() + Send + 'static>(&self, f: F) -> Result<(), TryExecuteError> {
        let tx = self.sender().ok_or(TryExecuteError::Shutdown)?;
        tx.try_send(Box::new(f)).map_err(|e| match e {
            TrySendError::Full(_) => TryExecuteError::Full,
            TrySendError::Disconnected(_) => TryExecuteError::Shutdown,
        })
    }

    /// Stops accepting jobs. Does not wait for anything. Idempotent.
    ///
    /// Queued jobs still run. Workers exit once the queue is empty.
    pub fn close(&self) {
        self.sender.lock().expect("sender lock poisoned").take();
    }

    /// Number of jobs that panicked.
    pub fn panicked_jobs(&self) -> usize {
        self.panicked.load(Ordering::Relaxed)
    }

    /// Closes the pool, runs every job already queued, then joins all workers.
    pub fn shutdown(self) {
        drop(self);
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        self.close();
        let handles = std::mem::take(&mut *self.workers.lock().expect("workers lock poisoned"));
        for handle in handles {
            let _ = handle.join();
        }
    }
}
