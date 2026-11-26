use ex02_lru_cache::{LruCache, ZeroCapacity};
use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn cache(cap: usize) -> LruCache<u32, u32> {
    LruCache::new(cap).unwrap()
}

/// Keys from most to least recently used.
fn order(c: &LruCache<u32, u32>) -> Vec<u32> {
    c.iter().map(|(k, _)| *k).collect()
}

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

#[test]
fn clear_empties_the_cache_and_it_stays_usable() {
    let mut c = cache(2);
    c.put(1, 10);
    c.put(2, 20);
    c.clear();
    assert!(c.is_empty());
    assert_eq!((c.capacity(), c.slab_len()), (2, 0));
    assert_eq!(c.get(&1), None);
    assert_eq!(c.put(3, 30), None);
    assert_eq!(c.put(4, 40), None);
    assert_eq!(c.put(5, 50), Some((3, 30)));
    assert_eq!(order(&c), vec![5, 4]);
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

/// Increments a shared counter when dropped.
struct Tracked {
    drops: Arc<AtomicUsize>,
}

impl Drop for Tracked {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

struct Tracker {
    drops: Arc<AtomicUsize>,
    created: usize,
}

impl Tracker {
    fn new() -> Self {
        Tracker {
            drops: Arc::new(AtomicUsize::new(0)),
            created: 0,
        }
    }

    fn make(&mut self) -> Tracked {
        self.created += 1;
        Tracked {
            drops: Arc::clone(&self.drops),
        }
    }

    fn dropped(&self) -> usize {
        self.drops.load(Ordering::SeqCst)
    }
}

#[test]
fn eviction_returns_the_value_and_drops_it_once() {
    let mut t = Tracker::new();
    let mut c = LruCache::new(1).unwrap();
    let v = t.make();
    c.put(1, v);
    let v = t.make();
    let evicted = c.put(2, v);
    assert_eq!(t.dropped(), 0, "evicted value is handed back, not dropped");
    drop(evicted);
    assert_eq!(t.dropped(), 1);
    drop(c);
    assert_eq!(t.dropped(), 2);
}

#[test]
fn remove_hands_back_the_value() {
    let mut t = Tracker::new();
    let mut c = LruCache::new(2).unwrap();
    let v = t.make();
    c.put(1, v);
    let removed = c.remove(&1);
    assert_eq!(t.dropped(), 0);
    drop(removed);
    assert_eq!(t.dropped(), 1);
    drop(c);
    assert_eq!(t.dropped(), 1);
}

#[test]
fn replace_drops_the_old_value_immediately() {
    let mut t = Tracker::new();
    let mut c = LruCache::new(2).unwrap();
    let v = t.make();
    c.put(1, v);
    let v = t.make();
    assert!(c.put(1, v).is_none());
    assert_eq!(t.dropped(), 1);
    drop(c);
    assert_eq!(t.dropped(), 2);
}

#[test]
fn clear_drops_every_value_once() {
    let mut t = Tracker::new();
    let mut c = LruCache::new(4).unwrap();
    for k in 0..4 {
        let v = t.make();
        c.put(k, v);
    }
    c.clear();
    assert_eq!(t.dropped(), 4);
    drop(c);
    assert_eq!(t.dropped(), 4);
}

#[test]
fn dropping_the_cache_drops_live_values_but_not_vacated_slots_twice() {
    let mut t = Tracker::new();
    let mut c = LruCache::new(4).unwrap();
    for k in 0..4 {
        let v = t.make();
        c.put(k, v);
    }
    drop(c.remove(&1));
    drop(c.remove(&2));
    assert_eq!(t.dropped(), 2);
    drop(c);
    assert_eq!(t.dropped(), 4);
}

#[test]
fn every_value_is_dropped_exactly_once_over_mixed_operations() {
    let mut t = Tracker::new();
    let mut c = LruCache::new(5).unwrap();
    for i in 0..1_000u32 {
        let k = i % 13;
        match i % 4 {
            0 | 1 => {
                let v = t.make();
                drop(c.put(k, v));
            }
            2 => drop(c.remove(&k)),
            _ => {
                c.get(&k);
            }
        }
    }
    drop(c);
    assert_eq!(t.dropped(), t.created);
}

#[derive(Debug, Clone)]
enum Op {
    Put(u8, u32),
    Get(u8),
    GetMut(u8, u32),
    Peek(u8),
    Remove(u8),
    Clear,
}

fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => (0u8..20, any::<u32>()).prop_map(|(k, v)| Op::Put(k, v)),
        3 => (0u8..20).prop_map(Op::Get),
        1 => (0u8..20, any::<u32>()).prop_map(|(k, v)| Op::GetMut(k, v)),
        1 => (0u8..20).prop_map(Op::Peek),
        2 => (0u8..20).prop_map(Op::Remove),
        1 => Just(Op::Clear),
    ]
}

/// Reference model: front of the Vec is the most recently used entry.
struct Model {
    cap: usize,
    items: Vec<(u8, u32)>,
}

impl Model {
    fn position(&self, k: u8) -> Option<usize> {
        self.items.iter().position(|(key, _)| *key == k)
    }

    fn touch(&mut self, k: u8) -> Option<&mut u32> {
        let pos = self.position(k)?;
        let item = self.items.remove(pos);
        self.items.insert(0, item);
        Some(&mut self.items[0].1)
    }

    fn put(&mut self, k: u8, v: u32) -> Option<(u8, u32)> {
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

fn check_ops(cap: usize, ops: &[Op]) -> Result<(), TestCaseError> {
    let mut real: LruCache<u8, u32> = LruCache::new(cap).unwrap();
    let mut model = Model {
        cap,
        items: Vec::new(),
    };
    for op in ops {
        match op {
            Op::Put(k, v) => prop_assert_eq!(real.put(*k, *v), model.put(*k, *v)),
            Op::Get(k) => prop_assert_eq!(real.get(k).copied(), model.touch(*k).map(|v| *v)),
            Op::GetMut(k, v) => {
                let r = real.get_mut(k).map(|slot| std::mem::replace(slot, *v));
                let m = model.touch(*k).map(|slot| std::mem::replace(slot, *v));
                prop_assert_eq!(r, m);
            }
            Op::Peek(k) => {
                let m = model.position(*k).map(|p| model.items[p].1);
                prop_assert_eq!(real.peek(k).copied(), m);
            }
            Op::Remove(k) => {
                let m = model.position(*k).map(|p| model.items.remove(p).1);
                prop_assert_eq!(real.remove(k), m);
            }
            Op::Clear => {
                real.clear();
                model.items.clear();
            }
        }
        let got: Vec<(u8, u32)> = real.iter().map(|(k, v)| (*k, *v)).collect();
        prop_assert_eq!(&got, &model.items);
        prop_assert_eq!(real.len(), model.items.len());
        prop_assert!(real.slab_len() <= cap);
    }
    Ok(())
}

#[test]
fn matches_naive_model_on_random_operation_sequences() {
    // A fixed RNG seed makes every run explore the same 256 cases.
    let config = Config {
        cases: 256,
        failure_persistence: None,
        ..Config::default()
    };
    let rng = TestRng::from_seed(RngAlgorithm::ChaCha, &[7u8; 32]);
    let mut runner = TestRunner::new_with_rng(config, rng);
    let strategy = (1usize..10, prop::collection::vec(op_strategy(), 0..=500));
    runner
        .run(&strategy, |(cap, ops)| check_ops(cap, &ops))
        .unwrap();
}

#[test]
#[ignore = "run with: cargo test --release -- --ignored --nocapture"]
fn throughput_one_million_mixed_operations() {
    let mut c: LruCache<u64, u64> = LruCache::new(1024).unwrap();
    // xorshift64: deterministic keys without a dependency.
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let ops = 1_000_000u64;
    let mut hits = 0u64;
    let start = std::time::Instant::now();
    for i in 0..ops {
        let r = next();
        let key = r % 4096;
        match r >> 60 {
            0..=9 => hits += c.get(&key).is_some() as u64,
            10..=14 => {
                c.put(key, i);
            }
            _ => {
                c.remove(&key);
            }
        }
    }
    let elapsed = start.elapsed();
    let per_sec = ops as f64 / elapsed.as_secs_f64();
    println!("{ops} ops in {elapsed:?} = {per_sec:.0} ops/sec, {hits} hits");
    assert!(elapsed.as_secs() < 5, "took {elapsed:?}");
}
