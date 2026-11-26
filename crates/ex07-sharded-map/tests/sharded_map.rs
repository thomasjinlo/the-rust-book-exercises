use ex07_sharded_map::ShardedMap;
use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Runs `body` on its own thread and fails if it panics (including `todo!()`) or
/// does not finish in `secs`. A hung thread is abandoned.
fn watchdog(secs: u64, body: impl FnOnce() + Send + 'static) {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        body();
        let _ = tx.send(());
    });
    match rx.recv_timeout(Duration::from_secs(secs)) {
        Ok(()) => {}
        Err(mpsc::RecvTimeoutError::Timeout) => panic!("no result after {secs}s: deadlock?"),
        Err(mpsc::RecvTimeoutError::Disconnected) => panic!("test body panicked"),
    }
}

#[test]
fn insert_returns_the_previous_value() {
    watchdog(30, || {
        let m = ShardedMap::new(4);
        assert_eq!(m.insert("a", 1), None);
        assert_eq!(m.insert("a", 2), Some(1));
        assert_eq!(m.insert("b", 3), None);
    });
}

#[test]
fn get_returns_a_copy_or_none() {
    watchdog(30, || {
        let m = ShardedMap::new(4);
        assert_eq!(m.get(&"a"), None);
        m.insert("a", String::from("x"));
        assert_eq!(m.get(&"a"), Some(String::from("x")));
        assert_eq!(m.get(&"b"), None);
    });
}

#[test]
fn update_stores_the_default_then_applies_the_closure() {
    watchdog(30, || {
        let m = ShardedMap::new(4);
        m.insert("hits", 2);
        let r = m.update(
            "hits",
            || 0,
            |v| {
                *v += 10;
                *v
            },
        );
        assert_eq!(r, 12);
        let r = m.update(
            "new",
            || 100,
            |v| {
                *v += 1;
                *v
            },
        );
        assert_eq!(r, 101);
        assert_eq!(m.get(&"hits"), Some(12));
        assert_eq!(m.get(&"new"), Some(101));
    });
}

#[test]
fn update_does_not_call_default_for_an_existing_key() {
    watchdog(30, || {
        let m = ShardedMap::new(4);
        m.insert(1, 5);
        let mut calls = 0;
        m.update(
            1,
            || {
                calls += 1;
                0
            },
            |v| *v += 1,
        );
        assert_eq!(calls, 0);
        assert_eq!(m.get(&1), Some(6));
    });
}

#[test]
fn any_shard_count_gives_the_same_results() {
    watchdog(30, || {
        for shards in [1, 2, 3, 7, 64, 1000] {
            let m = ShardedMap::new(shards);
            for k in 0..200u32 {
                assert_eq!(m.insert(k, k * 2), None);
            }
            for k in 0..200u32 {
                assert_eq!(m.get(&k), Some(k * 2), "shards = {shards}");
            }
            assert_eq!(m.get(&200), None);
        }
    });
}

/// Same answers as `std::collections::HashMap` on random calls from one thread.
#[test]
fn random_calls_match_std_hashmap() {
    watchdog(60, || {
        let mut rng = 0x2545_F491_4F6C_DD1Du64;
        let mut next = move || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        for shards in [1, 3, 64] {
            let m = ShardedMap::new(shards);
            let mut model: HashMap<u64, u64> = HashMap::new();
            for step in 0..5_000u64 {
                let r = next();
                let key = r % 50;
                match (r >> 60) & 3 {
                    0 => assert_eq!(m.insert(key, step), model.insert(key, step)),
                    1 => assert_eq!(m.get(&key), model.get(&key).copied()),
                    _ => {
                        let got = m.update(
                            key,
                            || step,
                            |v| {
                                *v += 1;
                                *v
                            },
                        );
                        let v = model.entry(key).or_insert(step);
                        *v += 1;
                        assert_eq!(got, *v);
                    }
                }
            }
        }
    });
}

#[test]
fn concurrent_updates_never_lose_a_write() {
    watchdog(60, || {
        let m = ShardedMap::new(64);
        thread::scope(|s| {
            for _ in 0..8 {
                s.spawn(|| {
                    for i in 0..100_000u64 {
                        m.update(i % 64, || 0u64, |v| *v += 1);
                    }
                });
            }
        });
        let total: u64 = (0..64).map(|k| m.get(&k).unwrap()).sum();
        assert_eq!(total, 800_000);
    });
}

#[test]
fn concurrent_inserts_of_different_keys_all_stay() {
    watchdog(60, || {
        let m = ShardedMap::new(8);
        thread::scope(|s| {
            for t in 0..4u32 {
                let m = &m;
                s.spawn(move || {
                    for i in 0..1_000 {
                        assert_eq!(m.insert(t * 1_000 + i, t), None);
                    }
                });
            }
        });
        for t in 0..4u32 {
            for i in 0..1_000 {
                assert_eq!(m.get(&(t * 1_000 + i)), Some(t));
            }
        }
    });
}

/// 8 threads must finish the same total number of `update` calls faster than 1 thread.
///
/// Run with `cargo test --release -- --ignored`.
#[test]
#[ignore]
fn eight_threads_beat_one_thread() {
    watchdog(120, || {
        const TOTAL: u64 = 1_600_000;
        let run = |threads: u64| {
            let m = ShardedMap::new(64);
            let start = Instant::now();
            thread::scope(|s| {
                for t in 0..threads {
                    let m = &m;
                    s.spawn(move || {
                        for i in 0..TOTAL / threads {
                            let key = (t * 7919 + i) % 1024;
                            m.update(key, || 0u64, |v| *v = busy(*v));
                        }
                    });
                }
            });
            start.elapsed()
        };
        let one = run(1);
        let eight = run(8);
        // Release build, 64 cores: the sharded map takes about 440 ms with 1 thread and 90 ms with
        // 8 (4.9x faster). One Mutex<HashMap> takes about 430 ms with 1 thread and 950 ms with 8
        // (2.2x slower), so it fails.
        assert!(eight < one, "1 thread: {one:?}, 8 threads: {eight:?}");
    });
}

/// Work done while the key is held, so time spent waiting for it shows up in the timing.
fn busy(mut x: u64) -> u64 {
    for _ in 0..100 {
        x = std::hint::black_box(x.wrapping_mul(6364136223846793005).wrapping_add(1));
    }
    x
}
