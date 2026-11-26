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
fn zero_capacity_is_an_error() {
    assert_eq!(LruCache::<u32, u32>::new(0).err(), Some(ZeroCapacity));
}

#[test]
fn empty_cache_reports_empty() {
    let mut c = cache(3);
    assert!(c.is_empty());
    assert_eq!((c.len(), c.capacity(), c.slab_len()), (0, 3, 0));
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
    assert_eq!((c.len(), c.slab_len()), (1, 1));
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
fn capacity_one() {
    let mut c = cache(1);
    assert_eq!(c.put(1, 10), None);
    assert_eq!(c.put(2, 20), Some((1, 10)));
    assert_eq!(c.get(&1), None);
    assert_eq!(c.get(&2), Some(&20));
    assert_eq!(c.put(3, 30), Some((2, 20)));
    assert_eq!(order(&c), vec![3]);
    assert_eq!(c.slab_len(), 1);
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
fn update_of_the_only_entry_keeps_one_entry() {
    let mut c = cache(1);
    c.put(1, 10);
    assert_eq!(c.put(1, 11), None);
    assert_eq!((c.len(), c.slab_len()), (1, 1));
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
fn iter_goes_from_most_to_least_recent_and_is_read_only() {
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

// Milestone 4: remove and the free list.

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
    assert_eq!(c.slab_len(), 3);
}

#[test]
fn remove_head_tail_and_last_entry() {
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
fn slab_fills_to_exactly_capacity() {
    let mut c = cache(5);
    for k in 0..50 {
        c.put(k, k);
    }
    assert_eq!(c.slab_len(), 5);
    for k in 45..50 {
        assert_eq!(c.remove(&k), Some(k));
    }
    assert_eq!(c.slab_len(), 5);
    for k in 100..105 {
        c.put(k, k);
    }
    assert_eq!(c.slab_len(), 5);
}

#[test]
fn slab_never_exceeds_capacity_over_a_million_put_remove_cycles() {
    let mut c = cache(8);
    for i in 0..1_000_000u32 {
        c.put(i, i);
        assert!(c.slab_len() <= 8);
        if i % 2 == 0 {
            assert_eq!(c.remove(&i), Some(i));
        }
    }
    assert!(c.slab_len() <= c.capacity());
    assert_eq!(c.len(), 8);
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
fn eviction_returns_the_value_and_drops_it_once() {
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
fn replace_drops_the_old_value_immediately() {
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
fn matches_naive_model_on_random_operation_sequences() {
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
            assert!(real.slab_len() <= cap);
        }
    }
}
