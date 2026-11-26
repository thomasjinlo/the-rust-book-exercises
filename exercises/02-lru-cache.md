# Exercise 02: O(1) LRU cache without `unsafe`

**Time:** ~2.5 h | **Area:** systems programming / data structures for caches | **Crate:** `crates/ex02-lru-cache`
**Builds on:** none (standalone)

## Read first
- [Rust Book ch04: ownership](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html)
- [Rust Book ch08: collections](https://doc.rust-lang.org/book/ch08-00-common-collections.html) (`Vec` and `HashMap`)
- [Rust Book ch10: generics and traits](https://doc.rust-lang.org/book/ch10-00-generics.html)
- [Rust Book ch11: testing](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Rust Book ch15: smart pointers](https://doc.rust-lang.org/book/ch15-00-smart-pointers.html) (read it to see why this exercise avoids `Rc<RefCell<_>>`)
- std: [`std::mem::replace`](https://doc.rust-lang.org/std/mem/fn.replace.html), [`Option::take`](https://doc.rust-lang.org/std/option/enum.Option.html#method.take)

## What you are building
A least-recently-used cache is a bounded map that evicts the entry unused for the longest time. Databases, proxies, DNS resolvers and page caches use it in front of a slow store. Every operation must be O(1).

## Concepts
Two structures work together:
- a `HashMap<K, usize>` finds an entry's position in O(1);
- a doubly linked list keeps entries in recency order, so moving an entry to the front and evicting the back are O(1).

A pointer-linked list fights the borrow checker. Instead, nodes live in a `Vec` (a slab) and refer to each other by `usize` index. A removed slot goes on a free list and is reused, so the slab never grows past the capacity.

```
map:    1 -> slot 0     2 -> slot 1     3 -> slot 2
slots:  [0] key 1   prev=1       next=NIL      <- tail (least recent)
        [1] key 2   prev=2       next=0
        [2] key 3   prev=NIL     next=1        <- head (most recent)
order, most to least recent:  3, 2, 1
```

Worked example, capacity 3 (order is most to least recent):

| Operation | Result | Order after | `slab_len()` |
|---|---|---|---|
| `put(1)`, `put(2)`, `put(3)` | all `None` | 3, 2, 1 | 3 |
| `get(&1)` | `Some` | 1, 3, 2 | 3 |
| `put(4)` | `Some((2, ..))` (2 was least recent) | 4, 1, 3 | 3 |
| `remove(&1)` | `Some(..)`, slot goes on the free list | 4, 3 | 3 |
| `put(5)` | `None`, reuses the freed slot | 5, 4, 3 | 3 |

## Starting point
- Edit only `crates/ex02-lru-cache/src/lib.rs`. Replace each `todo!()`.
- Do not edit `tests/lru.rs` or the public signatures. Private fields, `Node` and `Slot` are yours to change.
- Run from `crates/ex02-lru-cache`: `cargo test`.
- The starter compiles. Unmodified, all 19 given tests fail with a `todo!()` panic.

## Milestones
| Stage | Requirements | Tests that pass at the end | Run |
|---|---|---|---|
| 1. Construct and store | R1, R2, R7 | `zero_capacity_is_an_error`, `empty_cache_reports_empty`, `put_then_get_returns_the_value` | `cargo test -- zero_ empty_ put_then` |
| 2. Evict by recency | R3, R5 | `evicts_least_recently_used_in_insertion_order`, `get_protects_an_entry_from_eviction`, `capacity_one` | `cargo test -- evicts get_protects capacity_one` |
| 3. Update, peek, `get_mut`, `iter` | R4, R5, R6, R9 | `update_*` (2), `peek_*`, `get_mut_*`, `iter_*` | `cargo test -- update_ peek_ get_mut_ iter_` |
| 4. Remove and the free list | R7, R8 | `remove_*` (2), `slab_*` (2) | `cargo test -- remove_ slab_` |
| 5. Drop exactly once | R10 | `eviction_returns_*`, `replace_drops_*`, `every_value_*` | `cargo test -- drop` |
| 6. Compare with a model | R1 to R9 | `matches_naive_model_on_random_operation_sequences` | `cargo test -- matches_` |

Each run lists several name filters after `--`. A test runs if its name contains any filter.

## Requirements
"MRU" is most recently used. "LRU" is least recently used.

1. **R1.** `new(0)` returns `Err(ZeroCapacity)`. `new(n)` for `n >= 1` returns an empty cache with `capacity() == n`, `len() == 0`, `slab_len() == 0`.
2. **R2.** `put(k, v)` with a key not in the cache and `len() < capacity()` inserts it as MRU and returns `None`.
3. **R3.** `put(k, v)` with a key not in the cache and `len() == capacity()` removes the LRU entry, inserts the new one as MRU, and returns `Some((lru_key, lru_value))`. The evicted value is returned, not dropped.
4. **R4.** `put(k, v)` with a key already in the cache replaces the value, makes the entry MRU, returns `None`, and never evicts. The old value is dropped before `put` returns.
5. **R5.** `get(&k)` returns `Some(&v)` and makes the entry MRU. A miss returns `None` and changes nothing. `get_mut` behaves the same but returns `Option<&mut V>`.
6. **R6.** `peek(&k)` returns the value without changing recency.
7. **R7.** `len()` is the number of live entries. `is_empty()` is `len() == 0`. `capacity()` never changes.
8. **R8.** `remove(&k)` returns `Some(v)` and unlinks the entry from any position (head, tail, middle, or only entry). A missing key returns `None`. The slot joins the free list and the next insertion reuses it.
9. **R9.** `slab_len()` is the length of the slab `Vec`. It never exceeds `capacity()`, including after 1,000,000 `put`/`remove` cycles. Evict before you allocate, and take slots from the free list before pushing. `iter()` yields `(&K, &V)` from MRU to LRU, visits each live entry once, and does not change recency.
10. **R10.** Every value passed to `put` is dropped exactly once: on replace, on cache drop, or by the caller after `put` (evict) or `remove` returns it. A vacated slab slot holds no value.
11. **R11.** `put`, `get`, `get_mut`, `peek` and `remove` are O(1) expected time. `iter` is O(n) in total.
12. **R12.** `K: Hash + Eq + Clone`. The crate `src/` contains no `unsafe`, `Rc`, `RefCell` or `Cell`.

## Common mistakes
- Forgetting to update `head` or `tail` when you unlink the first or last node. The `remove_head_tail_and_last_entry` test catches it.
- Updating recency before you know the key exists. A miss must change nothing.
- Pushing a new slot on every `put` instead of reusing the free list. `slab_len()` grows past `capacity()`.
- Leaving the old node's `prev` and `next` pointing at live nodes after a remove. The next `remove` then corrupts the list.
- Cloning the value instead of moving it. The model test and the drop tests show the difference.

## Given tests
All in `tests/lru.rs`: 19 tests, none ignored.

| Test | Requirement |
|---|---|
| `zero_capacity_is_an_error` | R1 |
| `empty_cache_reports_empty` | R1, R5, R7, R8 |
| `put_then_get_returns_the_value` | R2, R5 |
| `evicts_least_recently_used_in_insertion_order` | R2, R3 |
| `get_protects_an_entry_from_eviction` | R3, R5 |
| `capacity_one` | R3, R9 |
| `update_existing_replaces_value_without_eviction` | R4 |
| `update_of_the_only_entry_keeps_one_entry` | R4, R9 |
| `peek_does_not_change_recency` | R6 |
| `get_mut_updates_value_and_recency` | R5 |
| `iter_goes_from_most_to_least_recent_and_is_read_only` | R9 |
| `remove_then_reinsert` | R8, R9 |
| `remove_head_tail_and_last_entry` | R8 |
| `slab_fills_to_exactly_capacity` | R9 |
| `slab_never_exceeds_capacity_over_a_million_put_remove_cycles` | R9 |
| `eviction_returns_the_value_and_drops_it_once` | R3, R10 |
| `replace_drops_the_old_value_immediately` | R4, R10 |
| `every_value_is_dropped_exactly_once_over_mixed_operations` | R10 |
| `matches_naive_model_on_random_operation_sequences` | R2 to R9. 200 runs from a fixed seed. Each run: capacity 1 to 9, 300 operations (`put`, `get`, `remove`) on keys 0 to 19, compared with a `Vec` model after every step. |

## Acceptance
Run in `crates/ex02-lru-cache`:

```
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
grep -nE "unsafe|Rc<|RefCell|Cell<" src/lib.rs
```

Pass condition: the first three commands exit 0, `cargo test` reports `19 passed; 0 failed` for the `lru` test binary, and the `grep` prints nothing outside comments. The reference solution passes the 19 tests in about 1.5 s in debug mode.

## Extra tests to write
1. Capacity 2: `put(1)`, `put(2)`, `get(&1)`, `put(3)`. Expect eviction of key 2. Catches a `get` that forgets to relink.
2. `put` the same key 100 times in a cache of capacity 3. Expect `len() == 1` and `slab_len() == 1`. Catches an update path that allocates a new node.
3. Fill to capacity, `remove` every key in LRU-first order, then fill again. Expect `slab_len() == capacity()` and correct `iter` order. Catches a broken `tail` after removing the last node.
4. Fill to capacity, `remove` every key in MRU-first order. Expect `is_empty()` and an empty `iter()`. Catches a stale `head`.
5. A `String` key and a `Vec<u8>` value. Expect the same behavior as integers. Catches code that relies on `Copy`.
6. `get_mut` on a missing key, then `put` of a new key. Expect no change to order. Catches a recency update before the lookup succeeds.

## Stretch (optional)
1. **`clear()`.** Remove every entry, drop every value once, reset `slab_len()` to 0 and keep `capacity()`. Write the tests first.
2. **`get_or_insert_with(&mut self, K, impl FnOnce() -> V) -> &V`.** On a miss it inserts and evicts when full. The closure runs at most once and only on a miss.
3. **Sharded wrapper.** Build `ShardedLru<K, V>` holding 16 `Mutex<LruCache<K, V>>`, choosing a shard by key hash. Run 8 threads doing 1,000,000 mixed operations each against 1 shard and against 16 shards. Record both ops/sec figures.
