use ex06_thread_pool::{PoolError, ThreadPool, TryExecuteError};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Barrier, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// Every test body runs under this watchdog so a deadlocked pool fails the
// test instead of hanging `cargo test`. 10 s is far above any healthy run
// (milliseconds). A panic in `f` drops the sender and fails here too.
fn within<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(Duration::from_secs(10))
        .expect("test body panicked or exceeded the 10 s watchdog")
}

// Gate: threads calling `wait` block until `open` is called.
struct Gate {
    open: Mutex<bool>,
    cv: Condvar,
}

impl Gate {
    fn new() -> Arc<Gate> {
        Arc::new(Gate {
            open: Mutex::new(false),
            cv: Condvar::new(),
        })
    }
    fn wait(&self) {
        let mut open = self.open.lock().unwrap();
        while !*open {
            open = self.cv.wait(open).unwrap();
        }
    }
    fn open(&self) {
        *self.open.lock().unwrap() = true;
        self.cv.notify_all();
    }
}

// Polls until `cond` holds. The bound only matters when the pool is broken.
fn wait_until(cond: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(8);
    while !cond() {
        assert!(Instant::now() < deadline, "condition not reached in 8 s");
        thread::sleep(Duration::from_millis(1));
    }
}

fn counter() -> Arc<AtomicUsize> {
    Arc::new(AtomicUsize::new(0))
}

#[test]
fn new_rejects_zero_workers_and_zero_capacity() {
    within(|| {
        assert_eq!(ThreadPool::new(0, 4).err(), Some(PoolError::ZeroWorkers));
        assert_eq!(ThreadPool::new(4, 0).err(), Some(PoolError::ZeroCapacity));
        assert_eq!(ThreadPool::new(0, 0).err(), Some(PoolError::ZeroWorkers));
        ThreadPool::new(1, 1).unwrap().shutdown();
    });
}

#[test]
fn the_pool_can_be_shared_and_sent_between_threads() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ThreadPool>();
}

#[test]
fn two_jobs_run_at_the_same_time_on_a_pool_of_two_workers() {
    within(|| {
        let pool = ThreadPool::new(2, 4).unwrap();
        // Each job waits for the other, so this only finishes if both run at once.
        let rendezvous = Arc::new(Barrier::new(2));
        for _ in 0..2 {
            let rendezvous = Arc::clone(&rendezvous);
            pool.execute(move || {
                rendezvous.wait();
            })
            .unwrap();
        }
        pool.shutdown();
    });
}

#[test]
fn ten_thousand_jobs_each_run_exactly_once() {
    within(|| {
        let hits = counter();
        let pool = ThreadPool::new(4, 64).unwrap();
        for _ in 0..10_000 {
            let hits = Arc::clone(&hits);
            pool.execute(move || {
                hits.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap();
        }
        pool.shutdown();
        assert_eq!(hits.load(Ordering::SeqCst), 10_000);
    });
}

#[test]
fn with_all_workers_busy_exactly_capacity_more_jobs_are_accepted() {
    within(|| {
        let workers = 2;
        let capacity = 3;
        let pool = ThreadPool::new(workers, capacity).unwrap();
        let gate = Gate::new();
        // Main joins the barrier, so passing it proves every worker is inside a job.
        let started = Arc::new(Barrier::new(workers + 1));
        for _ in 0..workers {
            let (gate, started) = (Arc::clone(&gate), Arc::clone(&started));
            pool.execute(move || {
                started.wait();
                gate.wait();
            })
            .unwrap();
        }
        started.wait();
        // Both workers are busy, so exactly `capacity` jobs fit.
        for _ in 0..capacity {
            pool.try_execute(|| {}).unwrap();
        }
        assert_eq!(pool.try_execute(|| {}), Err(TryExecuteError::Full));
        gate.open();
        pool.shutdown();
    });
}

#[test]
fn close_returns_while_a_job_is_still_running() {
    within(|| {
        let pool = ThreadPool::new(1, 1).unwrap();
        let gate = Gate::new();
        let started = Arc::new(Barrier::new(2));
        {
            let (gate, started) = (Arc::clone(&gate), Arc::clone(&started));
            pool.execute(move || {
                started.wait();
                gate.wait();
            })
            .unwrap();
        }
        started.wait();
        // The job cannot finish until the gate opens, so a `close` that
        // waited for it would hang here.
        pool.close();
        gate.open();
        pool.shutdown();
    });
}

#[test]
fn a_blocked_execute_resumes_when_a_worker_frees_a_slot() {
    within(|| {
        let pool = Arc::new(ThreadPool::new(1, 1).unwrap());
        let gate = Gate::new();
        let started = Arc::new(Barrier::new(2));
        let ran = counter();
        {
            let (gate, started) = (Arc::clone(&gate), Arc::clone(&started));
            pool.execute(move || {
                started.wait();
                gate.wait();
            })
            .unwrap();
        }
        started.wait();
        let ran2 = Arc::clone(&ran);
        pool.execute(move || {
            ran2.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap();
        assert_eq!(pool.try_execute(|| {}), Err(TryExecuteError::Full));

        let (tx, rx) = mpsc::channel();
        let producer = {
            let (pool, ran) = (Arc::clone(&pool), Arc::clone(&ran));
            thread::spawn(move || {
                let r = pool.execute(move || {
                    ran.fetch_add(1, Ordering::SeqCst);
                });
                tx.send(r).unwrap();
            })
        };
        // A timeout here is the expected outcome. It can only hide a bug
        // (false pass), never cause a false failure.
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
        gate.open();
        assert_eq!(rx.recv_timeout(Duration::from_secs(8)), Ok(Ok(())));
        producer.join().unwrap();
        let pool = Arc::try_unwrap(pool).ok().unwrap();
        pool.shutdown();
        assert_eq!(ran.load(Ordering::SeqCst), 2);
    });
}

#[test]
fn shutdown_runs_every_accepted_job() {
    within(|| {
        let pool = ThreadPool::new(1, 100).unwrap();
        let gate = Gate::new();
        let started = Arc::new(Barrier::new(2));
        {
            let (gate, started) = (Arc::clone(&gate), Arc::clone(&started));
            pool.execute(move || {
                started.wait();
                gate.wait();
            })
            .unwrap();
        }
        started.wait();
        let ran = counter();
        for _ in 0..50 {
            let ran = Arc::clone(&ran);
            pool.execute(move || {
                ran.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap();
        }
        // The worker is held inside the first job, so all 50 are still queued
        // when the pool is closed.
        pool.close();
        gate.open();
        pool.shutdown();
        assert_eq!(ran.load(Ordering::SeqCst), 50);
    });
}

#[test]
fn dropping_the_pool_runs_every_accepted_job() {
    within(|| {
        let ran = counter();
        {
            let pool = ThreadPool::new(2, 16).unwrap();
            for _ in 0..16 {
                let ran = Arc::clone(&ran);
                pool.execute(move || {
                    ran.fetch_add(1, Ordering::SeqCst);
                })
                .unwrap();
            }
        }
        assert_eq!(ran.load(Ordering::SeqCst), 16);
    });
}

#[test]
fn submissions_after_close_are_rejected_from_any_thread() {
    within(|| {
        let pool = Arc::new(ThreadPool::new(2, 4).unwrap());
        pool.close();
        pool.close();
        let other = Arc::clone(&pool);
        let (e, t) = thread::spawn(move || (other.execute(|| {}), other.try_execute(|| {})))
            .join()
            .unwrap();
        assert_eq!(e, Err(PoolError::Shutdown));
        assert_eq!(t, Err(TryExecuteError::Shutdown));
    });
}

#[test]
fn panicking_jobs_are_counted_and_the_pool_keeps_working() {
    within(|| {
        let pool = ThreadPool::new(2, 8).unwrap();
        for _ in 0..100 {
            pool.execute(|| panic!("job failure expected by the test"))
                .unwrap();
        }
        let ran = counter();
        for _ in 0..100 {
            let ran = Arc::clone(&ran);
            pool.execute(move || {
                ran.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap();
        }
        // `shutdown` consumes the pool, so read the counter before it.
        wait_until(|| ran.load(Ordering::SeqCst) == 100 && pool.panicked_jobs() == 100);
        pool.shutdown();
    });
}

#[test]
fn eight_producers_submit_concurrently() {
    within(|| {
        let pool = Arc::new(ThreadPool::new(4, 16).unwrap());
        let ran = counter();
        let producers: Vec<_> = (0..8)
            .map(|_| {
                let (pool, ran) = (Arc::clone(&pool), Arc::clone(&ran));
                thread::spawn(move || {
                    for _ in 0..1_000 {
                        let ran = Arc::clone(&ran);
                        pool.execute(move || {
                            ran.fetch_add(1, Ordering::SeqCst);
                        })
                        .unwrap();
                    }
                })
            })
            .collect();
        for p in producers {
            p.join().unwrap();
        }
        Arc::try_unwrap(pool).ok().unwrap().shutdown();
        assert_eq!(ran.load(Ordering::SeqCst), 8_000);
    });
}

#[test]
fn one_worker_runs_jobs_in_fifo_order() {
    within(|| {
        let pool = ThreadPool::new(1, 8).unwrap();
        let log = Arc::new(Mutex::new(Vec::new()));
        for i in 0..500 {
            let log = Arc::clone(&log);
            pool.execute(move || log.lock().unwrap().push(i)).unwrap();
        }
        pool.shutdown();
        assert_eq!(*log.lock().unwrap(), (0..500).collect::<Vec<_>>());
    });
}

#[test]
fn two_hundred_pools_each_drain_on_shutdown() {
    within(|| {
        for round in 0..200usize {
            let workers = 1 + round % 4;
            let pool = ThreadPool::new(workers, 8).unwrap();
            let ran = counter();
            for _ in 0..40 {
                let ran = Arc::clone(&ran);
                pool.execute(move || {
                    ran.fetch_add(1, Ordering::SeqCst);
                })
                .unwrap();
            }
            pool.shutdown();
            assert_eq!(ran.load(Ordering::SeqCst), 40, "round {round}");
        }
    });
}

// Fixed-seed xorshift64 so failures reproduce.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn counting_job(attempted: &Arc<AtomicUsize>, panics: bool) -> impl FnOnce() + Send + 'static {
    let attempted = Arc::clone(attempted);
    move || {
        attempted.fetch_add(1, Ordering::SeqCst);
        if panics {
            panic!("job failure expected by the test");
        }
    }
}

#[test]
fn randomized_mixes_of_jobs_panics_and_submission_styles() {
    within(|| {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        for case in 0..40 {
            let workers = 1 + (rng.next() % 4) as usize;
            let capacity = 1 + (rng.next() % 8) as usize;
            let jobs = (rng.next() % 200) as usize;
            let pool = ThreadPool::new(workers, capacity).unwrap();
            let attempted = counter();
            let mut expected_panics = 0;
            for _ in 0..jobs {
                let panics = rng.next() % 5 < 1;
                expected_panics += usize::from(panics);
                if rng.next() & 1 == 0 {
                    pool.execute(counting_job(&attempted, panics)).unwrap();
                } else {
                    // A rejected closure is dropped, so rebuild it per retry.
                    while let Err(e) = pool.try_execute(counting_job(&attempted, panics)) {
                        assert_eq!(e, TryExecuteError::Full);
                        thread::yield_now();
                    }
                }
            }
            wait_until(|| pool.panicked_jobs() == expected_panics);
            pool.shutdown();
            assert_eq!(attempted.load(Ordering::SeqCst), jobs, "case {case}");
        }
    });
}
