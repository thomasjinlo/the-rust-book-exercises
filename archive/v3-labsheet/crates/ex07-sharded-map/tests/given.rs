use ex07_sharded_map::{ShardedMap, ZeroShards};
use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Runs `body` on its own thread and fails if it does not finish in `secs`.
/// A panic in `body` (including `todo!()`) also fails the test. A hung body
/// thread is abandoned; it dies when the test process exits. Timeouts are
/// at least 10x the time a correct implementation needs.
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
fn zero_shards_is_an_error() {
    assert_eq!(ShardedMap::<u32, u32>::new(0).err(), Some(ZeroShards));
}

#[test]
fn any_positive_shard_count_works() {
    watchdog(30, || {
        for shards in [1, 2, 3, 7, 64, 1000] {
            let m = ShardedMap::new(shards).unwrap();
            for i in 0..500u32 {
                assert_eq!(m.insert(i, i * 2), None);
            }
            assert_eq!(m.len(), 500, "shards {shards}");
            assert_eq!(m.get(&499), Some(998), "shards {shards}");
        }
    });
}

#[test]
fn insert_get_remove_return_the_expected_values() {
    watchdog(30, || {
        let m = ShardedMap::new(8).unwrap();
        assert!(m.is_empty());
        assert_eq!(m.insert("a".to_string(), 1), None);
        assert_eq!(m.insert("a".to_string(), 2), Some(1));
        assert_eq!(m.get(&"a".to_string()), Some(2));
        assert_eq!(m.get(&"b".to_string()), None);
        assert_eq!(m.len(), 1);
        assert!(!m.is_empty());
        assert_eq!(m.remove(&"a".to_string()), Some(2));
        assert_eq!(m.remove(&"a".to_string()), None);
        assert!(m.is_empty());
    });
}

#[test]
fn update_stores_the_default_then_applies_the_closure() {
    watchdog(30, || {
        let m = ShardedMap::new(4).unwrap();
        let r = m.update(
            7u32,
            || 100,
            |v| {
                *v += 1;
                *v
            },
        );
        assert_eq!(r, 101);
        let r = m.update(
            7u32,
            || unreachable!("the key exists, so default must not run"),
            |v| {
                *v += 1;
                *v
            },
        );
        assert_eq!(r, 102);
        assert_eq!(m.get(&7), Some(102));
        assert_eq!(m.len(), 1);
    });
}

#[test]
fn a_value_returned_by_get_does_not_block_writers_to_the_same_key() {
    watchdog(30, || {
        let m = ShardedMap::new(1).unwrap();
        m.insert(1u32, "old".to_string());
        let (got_tx, got_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        thread::scope(|s| {
            let m = &m;
            s.spawn(move || {
                let held = m.get(&1).unwrap();
                got_tx.send(()).unwrap();
                // Keep the value alive until the main thread has finished writing.
                release_rx.recv().unwrap();
                assert_eq!(held, "old");
            });
            got_rx.recv().unwrap();
            assert_eq!(m.insert(1, "new".to_string()), Some("old".to_string()));
            assert_eq!(m.remove(&1), Some("new".to_string()));
            release_tx.send(()).unwrap();
        });
    });
}

#[test]
fn concurrent_updates_never_lose_a_write() {
    watchdog(60, || {
        const THREADS: usize = 8;
        const OPS: usize = 100_000;
        const KEYS: usize = 64;
        let m = ShardedMap::new(16).unwrap();
        thread::scope(|s| {
            for _ in 0..THREADS {
                s.spawn(|| {
                    for i in 0..OPS {
                        m.update(i % KEYS, || 0u64, |v| *v += 1);
                    }
                });
            }
        });
        assert_eq!(m.len(), KEYS);
        let mut total = 0;
        for k in 0..KEYS {
            // Keys below OPS % KEYS are hit one extra time per thread.
            let want = THREADS * (OPS / KEYS + usize::from(k < OPS % KEYS));
            assert_eq!(m.get(&k), Some(want as u64), "key {k}");
            total += want;
        }
        assert_eq!(total, THREADS * OPS);
    });
}

#[test]
fn concurrent_pushes_to_one_key_keep_every_element() {
    watchdog(60, || {
        const THREADS: usize = 4;
        const PUSHES: usize = 10_000;
        let m = ShardedMap::new(8).unwrap();
        thread::scope(|s| {
            for _ in 0..THREADS {
                s.spawn(|| {
                    for _ in 0..PUSHES {
                        m.update("k".to_string(), Vec::new, |v: &mut Vec<u8>| v.push(1));
                    }
                });
            }
        });
        assert_eq!(m.get(&"k".to_string()).unwrap().len(), THREADS * PUSHES);
    });
}

#[test]
fn disjoint_inserts_and_removes_from_many_threads_leave_the_exact_contents() {
    watchdog(60, || {
        const THREADS: u64 = 8;
        const PER: u64 = 10_000;
        let m = ShardedMap::new(32).unwrap();
        thread::scope(|s| {
            for t in 0..THREADS {
                let m = &m;
                s.spawn(move || {
                    let base = t * PER;
                    for k in base..base + PER {
                        assert_eq!(m.insert(k, k), None);
                    }
                    // Remove the odd keys of this thread's own range.
                    for k in (base..base + PER).filter(|k| k % 2 == 1) {
                        assert_eq!(m.remove(&k), Some(k));
                    }
                });
            }
        });
        assert_eq!(m.len(), (THREADS * PER / 2) as usize);
        for k in 0..THREADS * PER {
            let want = (k % 2 == 0).then_some(k);
            assert_eq!(m.get(&k), want, "key {k}");
        }
    });
}

#[test]
fn readers_see_only_values_that_were_written() {
    watchdog(60, || {
        const KEYS: u64 = 50;
        let m = ShardedMap::new(8).unwrap();
        for k in 0..KEYS {
            m.insert(k, k * 1000);
        }
        thread::scope(|s| {
            // Writers only ever store k * 1000 + (0..10) under key k.
            for t in 0..2u64 {
                let m = &m;
                s.spawn(move || {
                    for i in 0..20_000u64 {
                        let k = (i + t) % KEYS;
                        m.insert(k, k * 1000 + i % 10);
                    }
                });
            }
            for _ in 0..4 {
                let m = &m;
                s.spawn(move || {
                    for i in 0..20_000u64 {
                        let k = i % KEYS;
                        let v = m.get(&k).expect("key is never removed");
                        assert!((k * 1000..k * 1000 + 10).contains(&v), "key {k} value {v}");
                    }
                });
            }
        });
        assert_eq!(m.len(), KEYS as usize);
    });
}

#[test]
fn matches_std_hashmap_on_random_operations() {
    watchdog(60, || {
        // xorshift64: fixed seed, no external crate.
        let mut x = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = move || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        for shards in [1, 3, 16] {
            let m = ShardedMap::new(shards).unwrap();
            let mut model = HashMap::new();
            for _ in 0..20_000 {
                let r = next();
                let key = (r >> 8) % 200;
                match r % 5 {
                    0 | 1 => assert_eq!(m.insert(key, r), model.insert(key, r)),
                    2 => assert_eq!(m.remove(&key), model.remove(&key)),
                    3 => {
                        m.update(key, || 1, |v| *v = v.wrapping_mul(3));
                        let e = model.entry(key).or_insert(1);
                        *e = e.wrapping_mul(3);
                    }
                    _ => assert_eq!(m.get(&key), model.get(&key).copied()),
                }
            }
            assert_eq!(m.len(), model.len());
            for (k, v) in &model {
                assert_eq!(m.get(k), Some(*v));
            }
        }
    });
}
