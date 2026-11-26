//! Fixed-size thread pool with a bounded job queue.
//!
//! Build it on `std::sync::mpsc::sync_channel`, as in the Rust Book chapter 21
//! design, plus a bound, panic isolation and `close`.

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
    // Add fields. `close(&self)` must be able to drop the sending half, so it
    // needs interior mutability. Add a `Drop` impl that behaves like `shutdown`.
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
    /// Queued jobs still run. Workers exit once the queue is empty.
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
