//! Fixed-size thread pool.
//!
//! Replace each `todo!()`. Keep the public signatures. Add any private fields, helpers and a `Drop` impl.

/// A pool of worker threads that run submitted closures.
pub struct ThreadPool {
    // Add your own fields.
}

impl ThreadPool {
    /// Starts `workers` threads. `workers` is at least 1.
    pub fn new(workers: usize) -> ThreadPool {
        let _ = workers;
        todo!()
    }

    /// Hands `f` to the pool. Returns without waiting for `f` to run.
    pub fn execute<F: FnOnce() + Send + 'static>(&self, f: F) {
        let _ = f;
        todo!()
    }

    /// Returns after every job submitted so far has run and all workers have stopped.
    pub fn shutdown(self) {
        todo!()
    }
}
