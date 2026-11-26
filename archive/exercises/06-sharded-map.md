# Exercise 06: Sharded concurrent map

**Time:** ~3 h | **Area:** concurrency / performance | **Crate:** `crates/ex06-sharded-map`
**Builds on:** none (standalone). Stretch item 2 ties to exercise 02 (optional).

## Read first
- [Rust Book ch16.3: Shared-State Concurrency](https://doc.rust-lang.org/book/ch16-03-shared-state.html)
- [Rust Book ch10: Generic Types, Traits, and Lifetimes](https://doc.rust-lang.org/book/ch10-00-generics.html) and [ch10.2: Traits](https://doc.rust-lang.org/book/ch10-02-traits.html) for bounds
- [Rust Book ch15: Smart Pointers](https://doc.rust-lang.org/book/ch15-00-smart-pointers.html)
- std: [`RwLock`](https://doc.rust-lang.org/std/sync/struct.RwLock.html), [`Mutex`](https://doc.rust-lang.org/std/sync/struct.Mutex.html), [`PoisonError`](https://doc.rust-lang.org/std/sync/struct.PoisonError.html), [`HashMap`](https://doc.rust-lang.org/std/collections/struct.HashMap.html)
- std: [`Hash`](https://doc.rust-lang.org/std/hash/trait.Hash.html), [`Hasher`](https://doc.rust-lang.org/std/hash/trait.Hasher.html), [`BuildHasher`](https://doc.rust-lang.org/std/hash/trait.BuildHasher.html) (see `hash_one`), [`RandomState`](https://doc.rust-lang.org/std/hash/struct.RandomState.html)
- std: [`std::thread::scope`](https://doc.rust-lang.org/std/thread/fn.scope.html)

## What you are building
A hash map that many threads can use at once. A single `Mutex<HashMap>` serializes every operation, so
throughput stops scaling when cores are added. This map splits the key space into N independent shards, each
a `RwLock<HashMap<K, V>>`. Threads that touch different shards never contend. Lock striping like this appears
in connection tables, session stores, caches and metrics registries.

## Starting point
- Edit: `crates/ex06-sharded-map/src/lib.rs`. The public API and doc comments are fixed. The private fields
  are a suggestion and may change.
- Do not edit: `tests/given.rs`, `tests/bench.rs`, `Cargo.toml` (there are no dependencies).
- Run from `crates/ex06-sharded-map`: `cargo test`. On the unmodified starter all 12 given tests fail in
  under a second (`todo!()` panics are caught by the watchdog helper in the tests).

## Requirements
Types: `ShardedMap<K, V>`, `ZeroShards` (unit struct), `MAX_SHARDS = 65_536`. Methods need `K: Hash + Eq`.
`ShardedMap` must be `Sync` when `K` and `V` are `Send + Sync`. Do not add `unsafe`.

**Construction**
- R1. `new(0)` returns `Err(ZeroShards)`.
- R2. `new(n)` with `n >= 1` creates `min(n, MAX_SHARDS)` rounded up to the next power of two shards.
  Examples: 1 -> 1, 3 -> 4, 5 -> 8, 17 -> 32, 100 -> 128, 65_537 -> 65_536, `usize::MAX` -> 65_536.
  `shard_count()` returns that number and must not overflow or allocate more than `MAX_SHARDS` shards.

**Sharding**
- R3. Each map owns one `RandomState`. The shard index of a key is derived from one 64-bit hash of the key,
  computed with `hash_one`, using its HIGH bits: `hash >> (64 - log2(shard_count))`.
- R4. With 1 shard the index is 0. `hash >> 64` is an overflow, so handle this case explicitly.
- R5. Each inner `HashMap` uses its own default hasher. Shard choice and bucket choice must not share a hash
  function instance.
- R6. Keys `0..100_000u64` inserted into a 16-shard map leave every shard with between 5,000 and 7,500 entries.

**Operations**
- R7. `insert` returns the previous value, or `None`. `remove` returns the removed value, or `None`.
- R8. `get` returns a clone of the value (`V: Clone`). The lock is released before `get` returns.
- R9. `with(&key, f)` calls `f` once under the shard READ lock and returns `Some(f(&value))`. If the key is
  absent it returns `None` and does not call `f`.
- R10. `update(key, default, f)` takes the shard WRITE lock once. If the key is absent it inserts
  `default()`; it then calls `f(&mut value)` and returns the result. `default` is not called if the key exists.
  Concurrent `update` calls on one key never lose a write.
- R11. If `f` or `default` in `update` panics, the panic propagates to the caller. If `default` ran, its value
  stays in the map.
- R12. `retain(f)` visits every entry exactly once, calling `f(&key, &mut value)` under that shard's write
  lock, and removes entries for which `f` returns `false`.
- R13. `len()` is the sum of shard lengths. Each shard is read-locked one at a time, so the result is NOT a
  snapshot. Under concurrent writes it can be a count that the map never held at any single instant. Say this
  in the doc comment of `len`. `is_empty()` is `len() == 0`.
- R14. `shard_sizes()` returns one entry count per shard, in shard-index order. Its length is `shard_count()`.

**Locking**
- R15. Lock-order rule: no method holds two shard locks at once. Each method locks one shard, finishes with
  it, releases it, and only then may lock another. State this rule in the module docs. It rules out deadlock
  on shard locks: a deadlock needs a thread that holds one lock while waiting for another, and no thread here
  ever does. Closures passed to `with`, `update` and `retain` run under a shard lock. Calling the same map from inside
  one is a caller error and may deadlock. Say so in the module docs.
- R16. Poisoning. A panic inside a closure poisons the shard's lock. Every lock acquisition must recover with
  `PoisonError::into_inner` (`lock.write().unwrap_or_else(PoisonError::into_inner)`), never `unwrap()` and
  never by propagating the error. After a panic in an `update` closure, every method keeps working on
  every key, including keys in the poisoned shard. Put the recovery in two private helpers, one for read
  guards and one for write guards, so it exists in one place. Why this is safe: the panic happens in user code
  between operations on the `HashMap`, not inside one, so the table is structurally intact.

## Given tests
All in `tests/given.rs` unless noted. 12 tests plus 1 ignored benchmark.

| Test | Requirement |
|---|---|
| `new_rejects_zero_shards` | R1 |
| `shard_count_rounds_up_to_power_of_two` | R2, R14 (table of 10 inputs, includes the clamp) |
| `single_thread_semantics` | R7, R8, R9, R13 |
| `update_inserts_default_then_applies_f` | R10 |
| `single_shard_map_works` | R4, R14 |
| `retain_filters_and_can_mutate` | R12 |
| `counter_update_is_exact_across_threads` | R10: 8 threads x 100,000 increments over 64 keys, exact per-key totals |
| `disjoint_insert_remove_ends_with_exact_len` | R7, R13: 8 threads on disjoint key ranges, exact final `len` |
| `len_and_retain_finish_while_writers_run` | R15: `len` and `retain` while 4 writers run, 10 s watchdog |
| `keys_spread_evenly_over_16_shards` | R3, R5, R6 |
| `panic_in_update_closure_does_not_poison_the_map` | R11, R16 |
| `matches_std_hashmap_on_random_operations` | R7, R10, R12: fixed-seed xorshift against `std::HashMap`, shard counts 1, 3, 16 |
| `bench.rs::throughput_sharded_vs_single_mutex` (`#[ignore]`) | benchmark, see Acceptance |

## Acceptance
1. `cargo test` exits 0 and reports `12 passed` for `given`. The benchmark shows as `1 ignored`.
2. `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` exit 0.
3. Benchmark, run once on an otherwise idle machine:
   `cargo test --release --test bench -- --ignored --nocapture`
   It runs 4 s and prints a table of operations per second for 1, 2, 4 and 8 threads, for your map with 64
   shards and for a single `Mutex<HashMap<u64, u64>>`, with 90% reads and 10% writes over 10,000 keys.
   Record the table in your notes. The test only asserts that both columns are non-zero.
4. In your notes, answer in two or three sentences each:
   - Does the `Mutex` column rise, stay flat or fall as threads increase? Why?
   - Does the sharded column rise with threads? What limits it at 8 threads on your machine (core count,
     `RwLock` reader-count cache-line traffic, allocation in `insert`)?
   - At 1 thread, which column is higher and why?

   Expected trend: the `Mutex` column does not scale with threads; the sharded column does, up to the core
   count. The exact numbers depend on the machine and are not checked.

## Extra tests to write
Put these in `tests/extra.rs`.
1. `new(1_000_000)` then `shard_count() == 65_536`. Catches a missing clamp (allocation blow-up).
2. Insert 1,000 keys into `new(2)`, check both entries of `shard_sizes()` are non-zero and sum to 1,000. Catches
   a shift off by one that routes everything to shard 0.
3. A key type whose `Hash` is constant (`state.write_u8(0)`). All entries land in one shard and every
   operation still returns correct values. Catches reliance on hash spread for correctness.
4. Panic inside a `retain` closure on a 1-shard map, then `len()`, `insert`, `get`. Catches a missed
   poison recovery in `retain`.
5. Panic inside the `default` closure of `update`, then check the key is absent and the map still works.
6. `with` on a missing key never calls the closure (use a counter). Catches calling `f` on a default.
7. `ShardedMap<String, Vec<u8>>` with 4 threads doing `update(.., Vec::new, |v| v.push(1))` on one key;
   final length equals the total pushes. Catches a read-then-write `update`.
8. Assert `fn is_sync<T: Sync>() {}; is_sync::<ShardedMap<String, u64>>();` compiles. Catches a field
   that makes the map `!Sync`.

## Stretch (optional)
1. Lock-free reads: replace each shard's `RwLock<HashMap>` with `arc_swap::ArcSwap<HashMap>` (add
   `arc-swap = "1"` to `Cargo.toml`). Reads load a snapshot without locking; writes clone the shard and swap.
   Rerun the benchmark and explain the 90/10 and a 50/50 result.
2. Per-shard LRU: cap each shard at `capacity / shard_count` entries and evict the least recently used entry
   (exercise 02). State what "least recently used" means across shards and why it is only approximate.
3. An `entry(key)` API returning an enum `Occupied`/`Vacant`. Decide how it holds the write guard and what
   that does to the lock-order rule in R15.
