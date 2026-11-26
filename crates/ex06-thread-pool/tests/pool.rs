use ex06_thread_pool::ThreadPool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Runs `body` on its own thread so a hang fails the test instead of stalling the run.
fn with_watchdog(limit: Duration, body: impl FnOnce() + Send + 'static) {
    let (done, wait) = channel();
    let handle = thread::spawn(move || {
        body();
        let _ = done.send(());
    });
    match wait.recv_timeout(limit) {
        Err(RecvTimeoutError::Timeout) => panic!("no result after {limit:?}"),
        _ => {
            if let Err(e) = handle.join() {
                std::panic::resume_unwind(e);
            }
        }
    }
}

const LIMIT: Duration = Duration::from_secs(10);

#[test]
fn every_job_runs() {
    with_watchdog(LIMIT, || {
        let count = Arc::new(AtomicUsize::new(0));
        let pool = ThreadPool::new(4);
        for _ in 0..1000 {
            let count = Arc::clone(&count);
            pool.execute(move || {
                count.fetch_add(1, Ordering::SeqCst);
            });
        }
        pool.shutdown();
        assert_eq!(count.load(Ordering::SeqCst), 1000);
    });
}

#[test]
fn every_job_runs_exactly_once() {
    with_watchdog(LIMIT, || {
        let hits: Arc<Vec<AtomicUsize>> = Arc::new((0..200).map(|_| AtomicUsize::new(0)).collect());
        let pool = ThreadPool::new(3);
        for i in 0..200 {
            let hits = Arc::clone(&hits);
            pool.execute(move || {
                hits[i].fetch_add(1, Ordering::SeqCst);
            });
        }
        pool.shutdown();
        assert!(hits.iter().all(|h| h.load(Ordering::SeqCst) == 1));
    });
}

#[test]
fn shutdown_waits_for_slow_jobs() {
    with_watchdog(LIMIT, || {
        let count = Arc::new(AtomicUsize::new(0));
        let pool = ThreadPool::new(2);
        for _ in 0..6 {
            let count = Arc::clone(&count);
            pool.execute(move || {
                thread::sleep(Duration::from_millis(30));
                count.fetch_add(1, Ordering::SeqCst);
            });
        }
        pool.shutdown();
        assert_eq!(count.load(Ordering::SeqCst), 6);
    });
}

#[test]
fn dropping_the_pool_waits_for_jobs() {
    with_watchdog(LIMIT, || {
        let count = Arc::new(AtomicUsize::new(0));
        {
            let pool = ThreadPool::new(2);
            for _ in 0..6 {
                let count = Arc::clone(&count);
                pool.execute(move || {
                    thread::sleep(Duration::from_millis(30));
                    count.fetch_add(1, Ordering::SeqCst);
                });
            }
        }
        assert_eq!(count.load(Ordering::SeqCst), 6);
    });
}

#[test]
fn one_worker_runs_jobs_in_submission_order() {
    with_watchdog(LIMIT, || {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let pool = ThreadPool::new(1);
        for i in 0..50 {
            let seen = Arc::clone(&seen);
            pool.execute(move || seen.lock().unwrap().push(i));
        }
        pool.shutdown();
        assert_eq!(*seen.lock().unwrap(), (0..50).collect::<Vec<_>>());
    });
}

#[test]
fn a_panicking_job_does_not_stop_the_worker() {
    with_watchdog(LIMIT, || {
        let count = Arc::new(AtomicUsize::new(0));
        let pool = ThreadPool::new(1);
        pool.execute(|| panic!("job failed on purpose"));
        for _ in 0..3 {
            let count = Arc::clone(&count);
            pool.execute(move || {
                count.fetch_add(1, Ordering::SeqCst);
            });
        }
        pool.shutdown();
        assert_eq!(count.load(Ordering::SeqCst), 3);
    });
}

#[test]
fn many_threads_can_submit_through_an_arc() {
    with_watchdog(LIMIT, || {
        let count = Arc::new(AtomicUsize::new(0));
        let pool = Arc::new(ThreadPool::new(2));
        let producers: Vec<_> = (0..4)
            .map(|_| {
                let (pool, count) = (Arc::clone(&pool), Arc::clone(&count));
                thread::spawn(move || {
                    for _ in 0..100 {
                        let count = Arc::clone(&count);
                        pool.execute(move || {
                            count.fetch_add(1, Ordering::SeqCst);
                        });
                    }
                })
            })
            .collect();
        for p in producers {
            p.join().unwrap();
        }
        Arc::into_inner(pool).unwrap().shutdown();
        assert_eq!(count.load(Ordering::SeqCst), 400);
    });
}

#[test]
fn four_jobs_that_wait_for_each_other_all_finish() {
    let start = Instant::now();
    // Each job blocks until all four are running. A correct pool finishes in about 1 ms.
    // A pool that runs jobs one at a time never finishes, so the 10 s watchdog fails it.
    with_watchdog(LIMIT, || {
        let barrier = Arc::new(Barrier::new(4));
        let count = Arc::new(AtomicUsize::new(0));
        let pool = ThreadPool::new(4);
        for _ in 0..4 {
            let (barrier, count) = (Arc::clone(&barrier), Arc::clone(&count));
            pool.execute(move || {
                barrier.wait();
                count.fetch_add(1, Ordering::SeqCst);
            });
        }
        pool.shutdown();
        assert_eq!(count.load(Ordering::SeqCst), 4);
    });
    assert!(start.elapsed() < LIMIT);
}
