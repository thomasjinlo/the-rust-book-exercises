//! Fixed-size thread pool with a bounded job queue.

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
/// Dropping the pool behaves like [`ThreadPool::shutdown`].
pub struct ThreadPool {
    // Add private fields, and a `Drop` impl.
}

impl ThreadPool {
    /// Starts `workers` threads. Fails with `ZeroWorkers`, then `ZeroCapacity`.
    pub fn new(workers: usize, queue_capacity: usize) -> Result<ThreadPool, PoolError> {
        let _ = (workers, queue_capacity);
        todo!()
    }

    /// Queues `f`. Blocks while the queue is full.
    ///
    /// Returns `Err(PoolError::Shutdown)` if the pool is closed. Every job for
    /// which this returns `Ok(())` runs exactly once.
    pub fn execute<F: FnOnce() + Send + 'static>(&self, f: F) -> Result<(), PoolError> {
        let _ = f;
        todo!()
    }

    /// Queues `f` without blocking.
    ///
    /// `Shutdown` takes precedence over `Full`.
    pub fn try_execute<F: FnOnce() + Send + 'static>(&self, f: F) -> Result<(), TryExecuteError> {
        let _ = f;
        todo!()
    }

    /// Stops accepting jobs. Does not wait for anything. Idempotent.
    ///
    /// Jobs already accepted still run.
    pub fn close(&self) {
        todo!()
    }

    /// Number of jobs that panicked.
    pub fn panicked_jobs(&self) -> usize {
        todo!()
    }

    /// Closes the pool, runs every job already queued, then joins all workers.
    pub fn shutdown(self) {
        todo!()
    }
}
