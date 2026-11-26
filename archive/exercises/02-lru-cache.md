# Exercise 02: O(1) LRU cache without `unsafe`

**Time:** ~3 h | **Area:** systems programming / data structures for caches | **Crate:** `crates/ex02-lru-cache`
**Builds on:** none (standalone). The finished cache can later back exercise 01's keyed limiter (optional extension, never required).

## Read first
- [Rust Book ch04: ownership](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html)
- [Rust Book ch08: collections](https://doc.rust-lang.org/book/ch08-00-common-collections.html)
- [Rust Book ch10: generics and traits](https://doc.rust-lang.org/book/ch10-00-generics.html)
- [Rust Book ch11: testing](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Rust Book ch15: smart pointers](https://doc.rust-lang.org/book/ch15-00-smart-pointers.html) (read it to see why this exercise avoids `Rc<RefCell<_>>`)
- std: [`HashMap`](https://doc.rust-lang.org/std/collections/struct.HashMap.html), [`Vec`](https://doc.rust-lang.org/std/vec/struct.Vec.html), [`std::mem::replace`](https://doc.rust-lang.org/std/mem/fn.replace.html), [`Option::take`](https://doc.rust-lang.org/std/option/enum.Option.html#method.take)
- [proptest book](https://proptest-rs.github.io/proptest/intro.html) (for the given property test)

## What you are building
A least-recently-used cache is a bounded map that evicts the entry unused for the longest time. It sits in front of slow stores in databases, proxies, DNS resolvers and page caches. Every operation must be O(1): a hash map finds the entry, and a doubly linked list orders entries by recency.

In Rust a pointer-linked list fights the borrow checker. You will avoid pointers: nodes live in a `Vec` (a slab) and link to each other by `usize` index. Vacated slots form a free list, so the slab never grows past the capacity. No `unsafe`, no `Rc`, no `RefCell`.

## Starting point
- Edit only `crates/ex02-lru-cache/src/lib.rs`. Replace each `todo!()`.
- Do not edit `tests/lru.rs` or the public signatures. Private fields, `Node` and `Slot` are yours to change.
- Run from `crates/ex02-lru-cache`: `cargo test`.
- The starter compiles. Unmodified, all 22 given tests fail with a `todo!()` panic.

## Requirements
Let "MRU" mean most recently used and "LRU" least recently used.

1. **R1.** `new(0)` returns `Err(ZeroCapacity)`. `new(n)` for `n >= 1` returns an empty cache with `capacity() == n`, `len() == 0`, `slab_len() == 0`.
2. **R2.** `put(k, v)` with a key not in the cache and `len() < capacity()` inserts it as MRU and returns `None`.
3. **R3.** `put(k, v)` with a key not in the cache and `len() == capacity()` removes the LRU entry, inserts the new one as MRU, and returns `Some((lru_key, lru_value))`. The evicted value is returned, not dropped.
4. **R4.** `put(k, v)` with a key already in the cache replaces the value, makes the entry MRU, returns `None`, and never evicts. The old value is dropped before `put` returns.
5. **R5.** `get(&k)` returns `Some(&v)` and makes the entry MRU. A miss returns `None` and changes nothing. `get_mut` behaves the same but returns `Option<&mut V>`.
6. **R6.** `peek(&k)` returns the value without changing recency.
7. **R7.** `remove(&k)` returns `Some(v)` and unlinks the entry from any position (head, tail, middle, or only entry). A missing key returns `None`. The slot joins the free list and is reused by the next insertion.
8. **R8.** `len()` is the number of live entries. `is_empty()` is `len() == 0`. `capacity()` never changes.
9. **R9.** `slab_len()` is the length of the slab `Vec`. It never exceeds `capacity()`, including after 1,000,000 `put`/`remove` cycles. Evict before you allocate, and take slots from the free list before pushing.
10. **R10.** `iter()` yields `(&K, &V)` from MRU to LRU, visits each live entry once, and does not change recency.
11. **R11.** `clear()` removes every entry, keeps `capacity()`, and sets `slab_len()` to 0. The cache is usable afterwards.
12. **R12.** Every value passed to `put` is dropped exactly once: on replace, on `clear`, on cache drop, or by the caller after `put` (evict) or `remove` returns it. A vacated slab slot holds no value.
13. **R13.** All operations are O(1) expected time, except `iter` (O(n) total) and `clear` (O(n)).
14. **R14.** `K: Hash + Eq + Clone`. The key is stored in the node and cloned once into the map on a new insert.
15. **R15.** The crate contains no `unsafe`, `Rc`, `RefCell`, `Box`-linked nodes or `Cell`.

## Given tests
All in `tests/lru.rs`: 22 run by default, 1 is `#[ignore]`.

| Test | Requirement |
|---|---|
| `zero_capacity_is_an_error` | R1 |
| `empty_cache_reports_empty` | R1, R5, R7, R8, R10 |
| `evicts_least_recently_used_in_insertion_order` | R2, R3 |
| `get_protects_an_entry_from_eviction` | R3, R5 |
| `get_mut_updates_value_and_recency` | R5 |
| `peek_does_not_change_recency` | R6 |
| `update_existing_replaces_value_without_eviction` | R4 |
| `update_of_the_only_entry_keeps_one_entry` | R4, R9 |
| `remove_then_reinsert` | R7, R9 |
| `remove_head_tail_and_last_entry` | R7 |
| `capacity_one` | R3, R9 |
| `iter_goes_from_most_to_least_recent_and_is_read_only` | R10 |
| `clear_empties_the_cache_and_it_stays_usable` | R11 |
| `slab_never_exceeds_capacity_over_a_million_put_remove_cycles` | R9 |
| `slab_fills_to_exactly_capacity` | R9 |
| `eviction_returns_the_value_and_drops_it_once` | R3, R12 |
| `remove_hands_back_the_value` | R7, R12 |
| `replace_drops_the_old_value_immediately` | R4, R12 |
| `clear_drops_every_value_once` | R11, R12 |
| `dropping_the_cache_drops_live_values_but_not_vacated_slots_twice` | R12 |
| `every_value_is_dropped_exactly_once_over_mixed_operations` | R12 |
| `matches_naive_model_on_random_operation_sequences` | R2 to R11. 256 cases from a fixed seed. Each case: capacity 1..10, up to 500 operations (`put`, `get`, `get_mut`, `peek`, `remove`, `clear`) on keys 0..20, compared with a `Vec` model after every step. |
| `throughput_one_million_mixed_operations` (`#[ignore]`) | R13. Prints ops/sec. Asserts only that 1,000,000 mixed operations finish in under 5 s. |

## Acceptance
Run in `crates/ex02-lru-cache`:

```
cargo test
cargo test --release -- --ignored --nocapture
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Pass condition: every command exits 0, and `cargo test` reports `22 passed; 0 failed; 1 ignored` for the `lru` test binary. Record the ops/sec line from the ignored test in your notes. The reference solution measured about 22 million ops/sec on one development machine in release mode, finishing in 0.05 s against a 5 s limit.

## Extra tests to write
1. Capacity 2, `put(1)`, `put(2)`, `get(&1)`, `put(3)`. Expect eviction of key 2. Catches a `get` that forgets to relink.
2. `put` the same key 100 times in a cache of capacity 3. Expect `len() == 1` and `slab_len() == 1`. Catches an update path that allocates a new node.
3. Fill to capacity, `remove` every key in LRU-first order, then fill again. Expect `slab_len() == capacity()` and correct `iter` order. Catches a broken `tail` after removing the last node.
4. Fill to capacity, `remove` every key in MRU-first order. Expect `is_empty()` and an empty `iter()`. Catches a stale `head`.
5. A `String` key and a `Vec<u8>` value. Expect the same behavior as integers. Catches code that accidentally relies on `Copy`.
6. `get_mut` on a missing key, then `put` of a new key. Expect no change to order. Catches a recency update before the lookup succeeds.
7. A value type whose `Drop` panics on the second call, run through `put` on the same key and `clear`. Catches a double drop that the counter test hides when drops cancel out.
8. `clear` then 1,000,000 `put`s. Expect `slab_len() <= capacity()`. Catches a free list not reset by `clear`.

## Stretch (optional)
1. **Hasher parameter.** Add `S: BuildHasher + Default` as a third type parameter with default `RandomState`, and `with_hasher`. Compare ops/sec using a fast non-cryptographic hasher against `RandomState`.
2. **`get_or_insert_with(&mut self, K, impl FnOnce() -> V) -> &V`.** On a miss it inserts and evicts when full. The closure runs at most once and only on a miss. Return a reference without a second hash lookup.
3. **Sharded wrapper.** Build `ShardedLru<K, V>` holding 16 `Mutex<LruCache<K, V>>`, choosing a shard by key hash. Run 8 threads doing 1,000,000 mixed operations each against 1 shard and against 16 shards. Record both ops/sec figures and explain the difference.
