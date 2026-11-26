# Exercise 07: Sharded concurrent map

**Time:** ~2.5 h | **Area:** concurrency / performance | **Crate:** `crates/ex07-sharded-map`
**Builds on:** none (standalone)

## Read first
- [Rust Book ch08.3: Hash Maps](https://doc.rust-lang.org/book/ch08-03-hash-maps.html)
- [Rust Book ch10: Generic Types, Traits, and Lifetimes](https://doc.rust-lang.org/book/ch10-00-generics.html), especially [ch10.1](https://doc.rust-lang.org/book/ch10-01-syntax.html) and [ch10.2](https://doc.rust-lang.org/book/ch10-02-traits.html) for trait bounds
- [Rust Book ch15: Smart Pointers](https://doc.rust-lang.org/book/ch15-00-smart-pointers.html)
- [Rust Book ch16.1: Threads](https://doc.rust-lang.org/book/ch16-01-threads.html)
- [Rust Book ch16.2: Message Passing](https://doc.rust-lang.org/book/ch16-02-message-passing.html)
- [Rust Book ch16.3: Shared-State Concurrency](https://doc.rust-lang.org/book/ch16-03-shared-state.html)
- [Rust Book ch16.4: `Sync` and `Send`](https://doc.rust-lang.org/book/ch16-04-extensible-concurrency-sync-and-send.html)
- std: [`RwLock`](https://doc.rust-lang.org/std/sync/struct.RwLock.html)
- std: [`std::hash`](https://doc.rust-lang.org/std/hash/index.html) (`Hash`, `Hasher`, `BuildHasher::hash_one`, `RandomState`)
- std: [`std::thread::scope`](https://doc.rust-lang.org/std/thread/fn.scope.html)

## What you are building
A hash map that many threads can use at once. A single `Mutex<HashMap>` serializes every operation, so
throughput stops scaling when cores are added. This map splits the key space into N independent shards, each
a `RwLock<HashMap<K, V>>`. Threads that touch different shards never contend. Lock striping like this appears
in connection tables, session stores, caches and metrics registries.

## Concepts
One lock, every thread queues on it:

```
 T1 T2 T3 T4          T1 T2 T3 T4
  \  |  |  /            |  |   \  |
   [ Mutex ]          [S0][S1][S2][S3]   N shards, one RwLock each
   [HashMap]          threads on different shards run in parallel
```

**Choosing a shard.** Hash the key once to 64 bits. Take the top `log2(N)` bits as the shard index.
With N = 16, `log2(N) = 4`, so the index is `hash >> 60`. Worked example:

```
hash  = 0xA3F0_0000_0000_0001
hash >> 60 = 0xA = 10    -> shard 10      (the low bits, 0x1, are not used)
```

Rule: use the high bits, not `hash % N`. High bits are the best mixed part of most hash functions, and
they are not the low bits that a `HashMap` uses to pick its own buckets.

**Why N is a power of two.** `log2(N)` is then a whole number of bits and the index is one shift. Rounding:
3 -> 4, 5 -> 8, 17 -> 32. With N = 1 the shift would be 64, which overflows `u64 >> 64`. Return index 0 instead.

**Atomic update.** `get` then `insert` loses writes: two threads both read 5 and both write 6. `update` locks the
shard for writing once and does find, insert-default and modify under that single lock.

**Locks and deadlock.** Deadlock needs a thread that holds one lock while waiting for another. If no method
holds two shard locks at once, that cannot happen. `len` therefore locks shards one at a time and its result is
not a snapshot.

## Starting point
- Edit: `crates/ex07-sharded-map/src/lib.rs`. The public API and doc comments are fixed. The private fields
  are a suggestion and may change.
- Do not edit: `tests/given.rs`, `tests/bench.rs`, `Cargo.toml` (there are no dependencies).
- Run from `crates/ex07-sharded-map`: `cargo test`. On the unmodified starter all 9 given tests fail in
  under a second (`todo!()` panics).

## Requirements
Types: `ShardedMap<K, V>` and `ZeroShards` (unit struct). Methods need `K: Hash + Eq`.
`ShardedMap` must be `Sync` when `K` and `V` are `Send + Sync`. Do not add `unsafe`.

- R1. `new(0)` returns `Err(ZeroShards)`.
- R2. `new(n)` with `n >= 1` creates `n.next_power_of_two()` shards. Examples: 1 -> 1, 3 -> 4, 5 -> 8,
  17 -> 32, 100 -> 128. Behavior for `n` above 2^20 is unspecified. `shard_count()` returns the number created.
- R3. `shard_sizes()` returns one entry count per shard, in shard-index order. Its length is `shard_count()`.
- R4. Each map owns one `RandomState`. The shard index of a key is `hash_one(key) >> (64 - log2(shard_count))`.
  With 1 shard the index is 0.
- R5. Each inner `HashMap` uses its own default hasher, not the map's `RandomState`.
- R6. Keys `0..100_000u64` inserted into a 16-shard map leave every shard with between 5,000 and 7,500 entries.
- R7. `insert` returns the previous value or `None`. `remove` returns the removed value or `None`.
- R8. `get` returns a clone of the value (`V: Clone`). The lock is released before `get` returns.
- R9. `update(key, default, f)` takes the shard write lock once. If the key is absent it inserts `default()`.
  It then calls `f(&mut value)` and returns the result. `default` is not called if the key exists.
  Concurrent `update` calls on one key never lose a write.
- R10. `len()` is the sum of shard lengths, read-locking one shard at a time. The doc comment of `len` says the
  result is not a snapshot. `is_empty()` is `len() == 0`.
- R11. Design rule: no method holds two shard locks at once. State it in the module docs.
  The module docs also state two caller-side facts: a closure passed to `update` runs under the shard lock
  and must not call the same map, and a panic inside a closure may poison a shard so that later calls on it
  may panic. No test checks R11; the starter docs already contain the text. Keep it accurate.

## Common mistakes
- `hash % N` or `hash & (N - 1)` instead of the high bits. R6 may still pass, but R4 is violated.
- `hash >> 64` when there is one shard. Debug builds panic with an overflow; release builds shift by 0 and
  index out of bounds.
- `update` implemented as `get` then `insert`. The counter test loses increments.
- Holding a shard guard while locking another, for example calling `self.len()` inside `update`.
- Returning a reference into the map from `get`. A guard cannot outlive the call, so clone the value.
- Sizing the `Vec` of shards with `n` instead of the rounded count.

## Milestones
Stop and verify after each stage.

1. **Construction** (R1, R2, R3). Struct fields, `new`, `shard_count`, `shard_sizes`. `shard_sizes` can read each
   shard's `len()` already. Run `cargo test new_rejects` and `cargo test shard_count_rounds`.
2. **Single-threaded operations** (R4, R5, R7, R8, R10). Write `shard_index`, then `insert`, `get`, `remove`,
   `len`. Run `cargo test single_`.
3. **`update`** (R9). Run `cargo test update_inserts`, then `cargo test matches_std`. The second compares the map
   with `std::HashMap` over 20,000 random operations on 1, 3 and 16 shards.
4. **Threads** (R9, R10). Run `cargo test -- counter_update disjoint_insert`. Both use 8 threads. A failure here
   with stage 3 green means a lost write or a wrong `len`.
5. **Distribution and benchmark** (R4, R6, R11). Run `cargo test keys_spread`, then the benchmark in Acceptance.

## Given tests
All in `tests/given.rs` unless noted. 9 tests plus 1 ignored benchmark.

| Test | Requirement |
|---|---|
| `new_rejects_zero_shards` | R1 |
| `shard_count_rounds_up_to_power_of_two` | R2, R3 (7 inputs) |
| `single_thread_semantics` | R7, R8, R10 |
| `update_inserts_default_then_applies_f` | R9 |
| `single_shard_map_works` | R4 (1 shard), R3 |
| `counter_update_is_exact_across_threads` | R9: 8 threads x 100,000 increments over 64 keys, exact per-key totals |
| `disjoint_insert_remove_ends_with_exact_len` | R7, R10: 8 threads on disjoint key ranges, exact final `len` |
| `keys_spread_evenly_over_16_shards` | R4, R5, R6 |
| `matches_std_hashmap_on_random_operations` | R7, R9, R10: fixed-seed xorshift against `std::HashMap`, shard counts 1, 3, 16 |
| `bench.rs::throughput_sharded_vs_single_mutex` (`#[ignore]`) | benchmark, see Acceptance |

## Acceptance
1. `cargo test` exits 0 and reports `9 passed` for `given`. The benchmark shows as `1 ignored`.
2. `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` exit 0.
3. Benchmark, run once on an otherwise idle machine:
   `cargo test --release --test bench -- --ignored --nocapture`
   It runs about 4 s and prints a table of operations per second for 1, 2, 4 and 8 threads, for your map with 64
   shards and for a single `Mutex<HashMap<u64, u64>>`, with 90% reads and 10% writes over 10,000 keys.
   Record the table in your notes. The test only asserts that both columns are non-zero.
4. In your notes, answer in two or three sentences each:
   - Does the `Mutex` column rise, stay flat or fall as threads increase? Why?
   - Does the sharded column rise with threads? What limits it at 8 threads on your machine?
   - At 1 thread, which column is higher and why?

   Expected trend: the `Mutex` column does not scale; the sharded column scales up to the core count.
   Numbers depend on the machine and are not checked.

## Extra tests to write
Put these in `tests/extra.rs`.
1. `new(1)`: `shard_sizes()` is `[n]` after `n` inserts. Catches a shift-by-64 overflow.
2. Insert 1,000 keys into `new(2)`; both `shard_sizes()` entries are non-zero and sum to 1,000. Catches an
   index that always picks shard 0.
3. A key type whose `Hash` writes a constant. All entries land in one shard and every operation still returns
   correct values. Catches reliance on hash spread for correctness.
4. `update` on an existing key never calls `default` (use a counter).
5. `ShardedMap<String, Vec<u8>>` with 4 threads doing `update(.., Vec::new, |v| v.push(1))` on one key; the final
   length equals the total pushes. Catches a read-then-write `update`.
6. `fn is_sync<T: Sync>() {}; is_sync::<ShardedMap<String, u64>>();` compiles. Catches a field that makes the
   map `!Sync`.

## Stretch (optional)
1. Lock-free reads: replace each shard's `RwLock<HashMap>` with `arc_swap::ArcSwap<HashMap>` (`arc-swap = "1"`).
   Rerun the benchmark and explain the 90/10 result.
2. Add `with(&key, f)` and `retain(f)` running closures under the shard lock. Decide what `retain` does to R11.
3. Add an `entry(key)` API returning `Occupied`/`Vacant`. Decide how it holds the write guard and what that does
   to R11.
