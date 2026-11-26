use ex06_sharded_map::{ShardedMap, ZeroShards};
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Runs `body` on its own thread and fails if it does not finish in `secs`.
/// A panic in `body` (including `todo!()`) also fails the test. A hung body
/// thread is abandoned; it dies when the test process exits.
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
fn new_rejects_zero_shards() {
    assert_eq!(ShardedMap::<u32, u32>::new(0).err(), Some(ZeroShards));
}

#[test]
fn shard_count_rounds_up_to_power_of_two() {
    let table = [
        (1, 1),
        (2, 2),
        (3, 4),
        (5, 8),
        (16, 16),
        (17, 32),
        (100, 128),
        (65_536, 65_536),
        (65_537, 65_536),
        (usize::MAX, 65_536),
    ];
    for (asked, want) in table {
        let m = ShardedMap::<u32, u32>::new(asked).unwrap();
        assert_eq!(m.shard_count(), want, "asked {asked}");
        assert_eq!(m.shard_sizes().len(), want, "asked {asked}");
    }
}

#[test]
fn single_thread_semantics() {
    watchdog(30, || {
        let m = ShardedMap::new(8).unwrap();
        assert!(m.is_empty());
        assert_eq!(m.insert("a".to_string(), 1), None);
        assert_eq!(m.insert("a".to_string(), 2), Some(1));
        assert_eq!(m.get(&"a".to_string()), Some(2));
        assert_eq!(m.get(&"b".to_string()), None);
        assert_eq!(m.with(&"a".to_string(), |v| *v * 10), Some(20));
        assert_eq!(m.with(&"b".to_string(), |_| unreachable!()), None);
        assert_eq!(m.len(), 1);
        assert_eq!(m.remove(&"a".to_string()), Some(2));
        assert_eq!(m.remove(&"a".to_string()), None);
        assert!(m.is_empty());
    });
}

#[test]
fn update_inserts_default_then_applies_f() {
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
            || unreachable!("key exists"),
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
fn single_shard_map_works() {
    watchdog(30, || {
        let m = ShardedMap::new(1).unwrap();
        for i in 0..1000u32 {
            m.insert(i, i);
        }
        assert_eq!(m.shard_sizes(), vec![1000]);
        assert_eq!(m.get(&999), Some(999));
    });
}

#[test]
fn retain_filters_and_can_mutate() {
    watchdog(30, || {
        let m = ShardedMap::new(8).unwrap();
        for i in 0..1000u32 {
            m.insert(i, i);
        }
        m.retain(|k, v| {
            *v += 1;
            k % 2 == 0
        });
        assert_eq!(m.len(), 500);
        assert_eq!(m.get(&10), Some(11));
        assert_eq!(m.get(&11), None);
    });
}

#[test]
fn counter_update_is_exact_across_threads() {
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
fn disjoint_insert_remove_ends_with_exact_len() {
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
        let want = (THREADS * PER / 2) as usize;
        assert_eq!(m.len(), want);
        assert_eq!(m.shard_sizes().iter().sum::<usize>(), want);
        assert_eq!(m.get(&2), Some(2));
        assert_eq!(m.get(&3), None);
    });
}

#[test]
fn len_and_retain_finish_while_writers_run() {
    // 10 s is far above the milliseconds this takes; only a deadlock reaches it.
    watchdog(10, || {
        let m = ShardedMap::new(8).unwrap();
        let stop = AtomicBool::new(false);
        thread::scope(|s| {
            for t in 0..4u64 {
                let (m, stop) = (&m, &stop);
                s.spawn(move || {
                    let mut i = 0u64;
                    while !stop.load(Ordering::Relaxed) {
                        let k = t * 1_000_000 + i % 5000;
                        m.insert(k, i);
                        m.update(k, || 0, |v| *v += 1);
                        m.remove(&(k + 1));
                        i += 1;
                    }
                });
            }
            for _ in 0..200 {
                let _ = m.len();
                m.retain(|k, _| k % 7 != 0);
            }
            stop.store(true, Ordering::Relaxed);
        });
    });
}

#[test]
fn keys_spread_evenly_over_16_shards() {
    let m = ShardedMap::new(16).unwrap();
    for k in 0..100_000u64 {
        m.insert(k, ());
    }
    let sizes = m.shard_sizes();
    assert_eq!(sizes.len(), 16);
    assert_eq!(sizes.iter().sum::<usize>(), 100_000);
    for (i, n) in sizes.iter().enumerate() {
        assert!((5_000..=7_500).contains(n), "shard {i} holds {n}");
    }
}

#[test]
fn panic_in_update_closure_does_not_poison_the_map() {
    watchdog(30, || {
        // One shard: the panicking key and every later key share the poisoned lock.
        let m = ShardedMap::new(1).unwrap();
        m.insert(1u32, 10u32);
        let r = catch_unwind(AssertUnwindSafe(|| {
            m.update(2, || 0, |_| panic!("boom"));
        }));
        assert!(r.is_err());
        assert_eq!(m.get(&1), Some(10));
        assert_eq!(m.insert(3, 30), None);
        let r = m.update(
            1,
            || 0,
            |v| {
                *v += 1;
                *v
            },
        );
        assert_eq!(r, 11);
        assert_eq!(m.remove(&3), Some(30));
        // The default was inserted before the closure ran.
        assert_eq!(m.get(&2), Some(0));
        assert_eq!(m.len(), 2);
        m.retain(|_, _| true);
        assert_eq!(m.shard_sizes(), vec![2]);
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
            m.retain(|k, _| k % 2 == 0);
            model.retain(|k, _| k % 2 == 0);
            assert_eq!(m.len(), model.len());
            for (k, v) in &model {
                assert_eq!(m.get(k), Some(*v));
            }
        }
    });
}
