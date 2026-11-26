use ex02_lru_cache::{LruCache, ZeroCapacity};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn cache(cap: usize) -> LruCache<u32, u32> {
    LruCache::new(cap).unwrap()
}

/// Keys from most to least recently used.
fn order(c: &LruCache<u32, u32>) -> Vec<u32> {
    c.iter().map(|(k, _)| *k).collect()
}

// Milestone 1: construction and basic put/get.

#[test]
fn a_cache_with_capacity_zero_cannot_be_created() {
    assert_eq!(LruCache::<u32, u32>::new(0).err(), Some(ZeroCapacity));
}

#[test]
fn empty_cache_reports_empty() {
    let mut c = cache(3);
    assert!(c.is_empty());
    assert_eq!((c.len(), c.capacity()), (0, 3));
    assert_eq!(c.get(&1), None);
    assert_eq!(c.peek(&1), None);
    assert_eq!(c.remove(&1), None);
    assert_eq!(c.iter().count(), 0);
}

#[test]
fn put_then_get_returns_the_value() {
    let mut c = cache(3);
    assert_eq!(c.put(1, 10), None);
    assert_eq!(c.get(&1), Some(&10));
    assert_eq!(c.len(), 1);
}

// Milestone 2: eviction order.

#[test]
fn evicts_least_recently_used_in_insertion_order() {
    let mut c = cache(3);
    assert_eq!(c.put(1, 10), None);
    assert_eq!(c.put(2, 20), None);
    assert_eq!(c.put(3, 30), None);
    assert_eq!(c.put(4, 40), Some((1, 10)));
    assert_eq!(c.put(5, 50), Some((2, 20)));
    assert_eq!(order(&c), vec![5, 4, 3]);
    assert_eq!(c.len(), 3);
}

#[test]
fn get_protects_an_entry_from_eviction() {
    let mut c = cache(3);
    c.put(1, 10);
    c.put(2, 20);
    c.put(3, 30);
    assert_eq!(c.get(&1), Some(&10));
    assert_eq!(c.put(4, 40), Some((2, 20)));
    assert_eq!(order(&c), vec![4, 1, 3]);
}

#[test]
fn a_cache_of_capacity_one_keeps_only_the_newest_entry() {
    let mut c = cache(1);
    assert_eq!(c.put(1, 10), None);
    assert_eq!(c.put(2, 20), Some((1, 10)));
    assert_eq!(c.get(&1), None);
    assert_eq!(c.get(&2), Some(&20));
    assert_eq!(c.put(3, 30), Some((2, 20)));
    assert_eq!(order(&c), vec![3]);
}

// Milestone 3: update, peek, get_mut, iter.

#[test]
fn update_existing_replaces_value_without_eviction() {
    let mut c = cache(2);
    c.put(1, 10);
    c.put(2, 20);
    assert_eq!(c.put(1, 11), None);
    assert_eq!(c.len(), 2);
    assert_eq!(c.peek(&1), Some(&11));
    assert_eq!(order(&c), vec![1, 2]);
    assert_eq!(c.put(3, 30), Some((2, 20)));
}

#[test]
fn updating_the_only_entry_keeps_one_entry() {
    let mut c = cache(1);
    c.put(1, 10);
    assert_eq!(c.put(1, 11), None);
    assert_eq!(c.len(), 1);
    assert_eq!(c.peek(&1), Some(&11));
}

#[test]
fn peek_does_not_change_recency() {
    let mut c = cache(3);
    c.put(1, 10);
    c.put(2, 20);
    c.put(3, 30);
    assert_eq!(c.peek(&1), Some(&10));
    assert_eq!(order(&c), vec![3, 2, 1]);
    assert_eq!(c.put(4, 40), Some((1, 10)));
}

#[test]
fn get_mut_updates_value_and_recency() {
    let mut c = cache(3);
    c.put(1, 10);
    c.put(2, 20);
    c.put(3, 30);
    *c.get_mut(&1).unwrap() += 5;
    assert_eq!(order(&c), vec![1, 3, 2]);
    assert_eq!(c.peek(&1), Some(&15));
    assert_eq!(c.get_mut(&9), None);
}

#[test]
fn iteration_goes_from_most_to_least_recent_and_does_not_change_recency() {
    let mut c = cache(4);
    for k in 1..=4 {
        c.put(k, k * 10);
    }
    c.get(&2);
    c.get(&1);
    let pairs: Vec<(u32, u32)> = c.iter().map(|(k, v)| (*k, *v)).collect();
    assert_eq!(pairs, vec![(1, 10), (2, 20), (4, 40), (3, 30)]);
    let _ = c.iter().count();
    assert_eq!(order(&c), vec![1, 2, 4, 3]);
}

// Milestone 4: remove.

#[test]
fn remove_then_reinsert() {
    let mut c = cache(3);
    c.put(1, 10);
    c.put(2, 20);
    c.put(3, 30);
    assert_eq!(c.remove(&2), Some(20));
    assert_eq!(c.remove(&2), None);
    assert_eq!(c.len(), 2);
    assert_eq!(order(&c), vec![3, 1]);
    assert_eq!(c.put(2, 21), None);
    assert_eq!(order(&c), vec![2, 3, 1]);
}

#[test]
fn remove_works_on_newest_oldest_and_last_remaining_entries() {
    let mut c = cache(3);
    c.put(1, 10);
    c.put(2, 20);
    c.put(3, 30);
    assert_eq!(c.remove(&3), Some(30));
    assert_eq!(c.remove(&1), Some(10));
    assert_eq!(order(&c), vec![2]);
    assert_eq!(c.remove(&2), Some(20));
    assert!(c.is_empty());
    assert_eq!(c.iter().count(), 0);
    c.put(7, 70);
    assert_eq!(order(&c), vec![7]);
}

#[test]
fn a_removed_key_can_be_inserted_again_after_the_cache_was_full() {
    let mut c = cache(5);
    for k in 0..50 {
        c.put(k, k);
    }
    for k in 45..50 {
        assert_eq!(c.remove(&k), Some(k));
    }
    assert!(c.is_empty());
    for k in 100..105 {
        assert_eq!(c.put(k, k), None);
    }
    assert_eq!(order(&c), vec![104, 103, 102, 101, 100]);
}

// Milestone 5: values are dropped exactly once.

/// Increments a shared counter when dropped.
struct Tracked {
    drops: Arc<AtomicUsize>,
}

impl Drop for Tracked {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

fn tracked(drops: &Arc<AtomicUsize>) -> Tracked {
    Tracked {
        drops: Arc::clone(drops),
    }
}

#[test]
fn an_evicted_value_is_handed_back_and_dropped_once_by_the_caller() {
    let drops = Arc::new(AtomicUsize::new(0));
    let mut c = LruCache::new(1).unwrap();
    c.put(1, tracked(&drops));
    let evicted = c.put(2, tracked(&drops));
    assert_eq!(
        drops.load(Ordering::SeqCst),
        0,
        "evicted value is handed back"
    );
    drop(evicted);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    drop(c);
    assert_eq!(drops.load(Ordering::SeqCst), 2);
}

#[test]
fn replacing_a_value_drops_the_old_one_immediately() {
    let drops = Arc::new(AtomicUsize::new(0));
    let mut c = LruCache::new(2).unwrap();
    c.put(1, tracked(&drops));
    assert!(c.put(1, tracked(&drops)).is_none());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    drop(c);
    assert_eq!(drops.load(Ordering::SeqCst), 2);
}

#[test]
fn every_value_is_dropped_exactly_once_over_mixed_operations() {
    let drops = Arc::new(AtomicUsize::new(0));
    let mut created = 0;
    let mut c = LruCache::new(5).unwrap();
    for i in 0..1_000u32 {
        let k = i % 13;
        match i % 4 {
            0 | 1 => {
                created += 1;
                drop(c.put(k, tracked(&drops)));
            }
            2 => drop(c.remove(&k)),
            _ => {
                c.get(&k);
            }
        }
    }
    drop(c);
    assert_eq!(drops.load(Ordering::SeqCst), created);
}

// Milestone 6: randomized comparison against a simple model.

/// Reference model: front of the Vec is the most recently used entry.
struct Model {
    cap: usize,
    items: Vec<(u32, u32)>,
}

impl Model {
    fn touch(&mut self, k: u32) -> Option<&mut u32> {
        let pos = self.items.iter().position(|(key, _)| *key == k)?;
        let item = self.items.remove(pos);
        self.items.insert(0, item);
        Some(&mut self.items[0].1)
    }

    fn put(&mut self, k: u32, v: u32) -> Option<(u32, u32)> {
        if let Some(slot) = self.touch(k) {
            *slot = v;
            return None;
        }
        let evicted = if self.items.len() == self.cap {
            self.items.pop()
        } else {
            None
        };
        self.items.insert(0, (k, v));
        evicted
    }
}

/// xorshift64: a fixed seed makes every run identical.
fn xorshift(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

#[test]
fn behaves_like_a_simple_model_on_random_operation_sequences() {
    let mut rng = 0x2545_F491_4F6C_DD1Du64;
    for _ in 0..200 {
        let cap = 1 + (xorshift(&mut rng) % 9) as usize;
        let mut real = cache(cap);
        let mut model = Model {
            cap,
            items: Vec::new(),
        };
        for step in 0..300u32 {
            let r = xorshift(&mut rng);
            let key = (r % 20) as u32;
            match (r >> 8) % 4 {
                0 | 1 => assert_eq!(real.put(key, step), model.put(key, step)),
                2 => assert_eq!(real.get(&key).copied(), model.touch(key).map(|v| *v)),
                _ => {
                    let pos = model.items.iter().position(|(k, _)| *k == key);
                    let expected = pos.map(|p| model.items.remove(p).1);
                    assert_eq!(real.remove(&key), expected);
                }
            }
            let got: Vec<(u32, u32)> = real.iter().map(|(k, v)| (*k, *v)).collect();
            assert_eq!(got, model.items);
            assert_eq!(real.len(), model.items.len());
        }
    }
}

// Speed (run with `cargo test --release -- --ignored`).

#[test]
#[ignore = "timing check; run in release mode"]
fn one_million_mixed_operations_on_capacity_1024_finish_within_5_seconds() {
    let mut rng = 0x9E37_79B9_7F4A_7C15u64;
    let mut c = cache(1024);
    let start = std::time::Instant::now();
    for i in 0..1_000_000u32 {
        let r = xorshift(&mut rng);
        let key = (r % 4096) as u32;
        match (r >> 16) % 4 {
            0 | 1 => {
                c.put(key, i);
            }
            2 => {
                c.get(&key);
            }
            _ => {
                c.remove(&key);
            }
        }
    }
    let elapsed = start.elapsed();
    eprintln!("1,000,000 mixed operations took {elapsed:?}");
    assert!(
        elapsed < std::time::Duration::from_secs(5),
        "took {elapsed:?}"
    );
}
