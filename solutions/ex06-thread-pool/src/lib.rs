//! Fixed-size thread pool.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

type Job = Box<dyn FnOnce() + Send + 'static>;

/// A pool of worker threads that run submitted closures.
pub struct ThreadPool {
    // Dropping the sender makes `recv` fail once the queue is empty, which is how workers
    // finish the remaining jobs and then exit.
    sender: Option<Sender<Job>>,
    workers: Vec<JoinHandle<()>>,
}

impl ThreadPool {
    /// Starts `workers` threads. `workers` is at least 1.
    pub fn new(workers: usize) -> ThreadPool {
        let (tx, rx) = channel::<Job>();
        let rx = Arc::new(Mutex::new(rx));
        let workers = (0..workers)
            .map(|_| {
                let rx = Arc::clone(&rx);
                thread::spawn(move || {
                    loop {
                        // A `while let` would keep the guard alive during the job and
                        // run jobs one at a time. The `let` drops it first.
                        let job = rx.lock().unwrap().recv();
                        let Ok(job) = job else { return };
                        let _ = catch_unwind(AssertUnwindSafe(job));
                    }
                })
            })
            .collect();
        ThreadPool {
            sender: Some(tx),
            workers,
        }
    }

    /// Hands `f` to the pool. Returns without waiting for `f` to run.
    pub fn execute<F: FnOnce() + Send + 'static>(&self, f: F) {
        // Workers only exit after the sender is dropped, so the send cannot fail here.
        let _ = self.sender.as_ref().unwrap().send(Box::new(f));
    }

    /// Returns after every job submitted so far has run and all workers have stopped.
    pub fn shutdown(self) {
        drop(self);
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        self.sender.take();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}
