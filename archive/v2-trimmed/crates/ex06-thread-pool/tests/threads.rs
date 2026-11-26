//! Own test binary: `/proc/self/task` counts every thread in the process, so
//! no other test may run threads beside this one.
#![cfg(target_os = "linux")]

use ex06_thread_pool::ThreadPool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

fn thread_count() -> usize {
    std::fs::read_dir("/proc/self/task").unwrap().count()
}

#[test]
fn dropping_the_pool_joins_every_worker() {
    // The watchdog thread is spawned first so it is part of the baseline.
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let before = thread_count();
        let ran = Arc::new(AtomicUsize::new(0));
        let pool = ThreadPool::new(8, 4).unwrap();
        assert_eq!(thread_count(), before + 8);
        for _ in 0..100 {
            let ran = Arc::clone(&ran);
            pool.execute(move || {
                ran.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap();
        }
        drop(pool);
        assert_eq!(ran.load(Ordering::SeqCst), 100);
        let _ = tx.send((before, thread_count()));
    });
    let (before, after) = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("test body panicked or exceeded the 10 s watchdog");
    assert_eq!(before, after);
}
