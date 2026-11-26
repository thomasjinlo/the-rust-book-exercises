//! Fixed-size thread pool with a bounded job queue.
//!
//! Queue: `Mutex<VecDeque<Job>>` plus `Condvar`s. No channels.

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

/// A pool of `workers` threads fed by a queue of at most `queue_capacity` jobs.
///
/// Jobs that are running do not count against the capacity.
/// The pool must be `Send + Sync` so it can be shared through `Arc`.
pub struct ThreadPool {
    // Add fields. Add a `Drop` impl that behaves like `shutdown`.
}

impl ThreadPool {
    /// Starts `workers` threads. Fails with `ZeroWorkers`, then `ZeroCapacity`.
    pub fn new(workers: usize, queue_capacity: usize) -> Result<ThreadPool, PoolError> {
        let _ = (workers, queue_capacity);
        todo!()
    }

    /// Queues `f`. Blocks while the queue is full.
    ///
    /// Returns `Err(PoolError::Shutdown)` if shutdown has begun, including when
    /// it begins while this call is blocked. In that case `f` is dropped unrun.
    pub fn execute<F: FnOnce() + Send + 'static>(&self, f: F) -> Result<(), PoolError> {
        let _ = f;
        todo!()
    }

    /// Queues `f` without blocking.
    pub fn try_execute<F: FnOnce() + Send + 'static>(&self, f: F) -> Result<(), TryExecuteError> {
        let _ = f;
        todo!()
    }

    /// Begins shutdown without waiting for it to finish.
    ///
    /// After this call `execute` and `try_execute` return `Shutdown`, blocked
    /// `execute` calls wake and return `Shutdown`, and workers exit once the
    /// queue is empty. Idempotent. Does not join workers.
    pub fn close(&self) {
        todo!()
    }

    /// Number of jobs waiting in the queue (not counting running jobs).
    pub fn queued(&self) -> usize {
        todo!()
    }

    /// Number of jobs that panicked.
    pub fn panicked_jobs(&self) -> usize {
        todo!()
    }

    /// Number of jobs that ran to completion without panicking.
    pub fn completed_jobs(&self) -> usize {
        todo!()
    }

    /// Closes the pool, runs every job already queued, then joins all workers.
    pub fn shutdown(self) {
        todo!()
    }
}
