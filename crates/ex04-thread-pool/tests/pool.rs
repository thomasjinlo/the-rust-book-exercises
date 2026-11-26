use ex04_thread_pool::{PoolError, ThreadPool, TryExecuteError};
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

// Channel-free gate: threads calling `wait` block until `open` is called.
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
fn new_rejects_zero_arguments() {
    within(|| {
        assert_eq!(ThreadPool::new(0, 4).err(), Some(PoolError::ZeroWorkers));
        assert_eq!(ThreadPool::new(4, 0).err(), Some(PoolError::ZeroCapacity));
        assert_eq!(ThreadPool::new(0, 0).err(), Some(PoolError::ZeroWorkers));
        ThreadPool::new(1, 1).unwrap().shutdown();
    });
}

#[test]
fn ten_thousand_jobs_counted_exactly() {
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
fn try_execute_reports_full_when_workers_busy_and_queue_at_capacity() {
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
        assert_eq!(pool.queued(), 0);
        for _ in 0..capacity {
            pool.try_execute(|| {}).unwrap();
        }
        assert_eq!(pool.queued(), capacity);
        assert_eq!(pool.try_execute(|| {}), Err(TryExecuteError::Full));
        assert_eq!(pool.queued(), capacity);
        gate.open();
        pool.shutdown();
    });
}

#[test]
fn blocked_execute_resumes_when_a_slot_frees() {
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
fn shutdown_runs_every_queued_job() {
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
        assert_eq!(pool.queued(), 50);
        // The worker is held inside the first job, so all 50 are still queued
        // when shutdown begins.
        pool.close();
        assert_eq!(pool.queued(), 50);
        gate.open();
        pool.shutdown();
        assert_eq!(ran.load(Ordering::SeqCst), 50);
    });
}

#[test]
fn shutdown_called_from_another_thread_drains_too() {
    within(|| {
        let pool = ThreadPool::new(2, 32).unwrap();
        let ran = counter();
        for _ in 0..32 {
            let ran = Arc::clone(&ran);
            pool.execute(move || {
                ran.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap();
        }
        thread::spawn(move || pool.shutdown()).join().unwrap();
        assert_eq!(ran.load(Ordering::SeqCst), 32);
    });
}

#[test]
fn drop_without_shutdown_drains() {
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
fn submissions_after_close_return_shutdown() {
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
fn close_wakes_a_blocked_producer_with_shutdown() {
    within(|| {
        let pool = Arc::new(ThreadPool::new(1, 1).unwrap());
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
        pool.execute(|| {}).unwrap();
        let unrun = counter();
        let producer = {
            let (pool, unrun) = (Arc::clone(&pool), Arc::clone(&unrun));
            thread::spawn(move || {
                pool.execute(move || {
                    unrun.fetch_add(1, Ordering::SeqCst);
                })
            })
        };
        // Whether the producer is already blocked or not yet inside `execute`,
        // the result is Shutdown.
        pool.close();
        assert_eq!(producer.join().unwrap(), Err(PoolError::Shutdown));
        gate.open();
        let pool = Arc::try_unwrap(pool).ok().unwrap();
        pool.shutdown();
        assert_eq!(unrun.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn panicking_jobs_do_not_kill_workers() {
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
        pool.close();
        wait_until(|| pool.completed_jobs() + pool.panicked_jobs() == 200);
        assert_eq!(pool.panicked_jobs(), 100);
        assert_eq!(pool.completed_jobs(), 100);
        pool.shutdown();
        assert_eq!(ran.load(Ordering::SeqCst), 100);
    });
}

#[test]
fn single_worker_with_all_workers_panicking_still_serves() {
    within(|| {
        let pool = ThreadPool::new(1, 2).unwrap();
        for _ in 0..10 {
            pool.execute(|| panic!("job failure expected by the test"))
                .unwrap();
        }
        let ran = counter();
        let ran2 = Arc::clone(&ran);
        pool.execute(move || {
            ran2.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap();
        pool.shutdown();
        assert_eq!(ran.load(Ordering::SeqCst), 1);
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
fn queue_never_exceeds_capacity() {
    within(|| {
        let pool = Arc::new(ThreadPool::new(2, 5).unwrap());
        let producers: Vec<_> = (0..4)
            .map(|_| {
                let pool = Arc::clone(&pool);
                thread::spawn(move || {
                    for _ in 0..500 {
                        pool.execute(|| {}).unwrap();
                        assert!(pool.queued() <= 5);
                    }
                })
            })
            .collect();
        for p in producers {
            p.join().unwrap();
        }
        Arc::try_unwrap(pool).ok().unwrap().shutdown();
    });
}

#[test]
fn stress_drain_200_times() {
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
            pool.close();
            wait_until(|| pool.completed_jobs() + pool.panicked_jobs() == jobs);
            assert_eq!(pool.panicked_jobs(), expected_panics, "case {case}");
            assert_eq!(pool.completed_jobs(), jobs - expected_panics, "case {case}");
            pool.shutdown();
            assert_eq!(attempted.load(Ordering::SeqCst), jobs, "case {case}");
        }
    });
}

#[test]
#[ignore = "throughput measurement; run with --release -- --ignored"]
fn throughput_one_million_empty_jobs() {
    let pool = ThreadPool::new(4, 1024).unwrap();
    let ran = counter();
    let start = Instant::now();
    for _ in 0..1_000_000 {
        let ran = Arc::clone(&ran);
        pool.execute(move || {
            ran.fetch_add(1, Ordering::Relaxed);
        })
        .unwrap();
    }
    pool.shutdown();
    let secs = start.elapsed().as_secs_f64();
    assert_eq!(ran.load(Ordering::Relaxed), 1_000_000);
    println!("1,000,000 jobs in {secs:.3} s = {:.0} jobs/s", 1e6 / secs);
}
