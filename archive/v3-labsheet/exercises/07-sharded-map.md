# Exercise 07: Sharded concurrent map

**Time:** 2.5 h | **Crate:** `crates/ex07-sharded-map`

## Read first
**Rust Book**
- [Ch. 8.3: Storing Keys with Associated Values in Hash Maps](https://doc.rust-lang.org/book/ch08-03-hash-maps.html)
- [Ch. 10.2: Defining Shared Behavior with Traits](https://doc.rust-lang.org/book/ch10-02-traits.html)
- [Ch. 16: Fearless Concurrency](https://doc.rust-lang.org/book/ch16-00-concurrency.html)
- [Ch. 16.1: Using Threads to Run Code Simultaneously](https://doc.rust-lang.org/book/ch16-01-threads.html)
- [Ch. 16.2: Transfer Data Between Threads with Message Passing](https://doc.rust-lang.org/book/ch16-02-message-passing.html)
- [Ch. 16.3: Shared-State Concurrency](https://doc.rust-lang.org/book/ch16-03-shared-state.html)
- [Ch. 16.4: Extensible Concurrency with Send and Sync](https://doc.rust-lang.org/book/ch16-04-extensible-concurrency-sync-and-send.html)

**Standard library**
- [`RwLock`](https://doc.rust-lang.org/std/sync/struct.RwLock.html)

**Background**
- [Striped (Guava: Google Core Libraries for Java API)](https://guava.dev/releases/snapshot-jre/api/docs/com/google/common/util/concurrent/Striped.html): why one lock for everything causes contention and how splitting it into independent locks lets unrelated operations run in parallel.
- [ConcurrentHashMap (Java Platform SE 7)](https://docs.oracle.com/javase/7/docs/api/java/util/concurrent/ConcurrentHashMap.html): a production concurrent hash map whose `concurrencyLevel` argument is a sizing hint, like `shards` here, and what that hint trades off.

## Task
Build a hash map that many threads use at once through a shared `&ShardedMap`. It must stay correct under
concurrent use and lose less throughput to waiting than one `Mutex<HashMap>`. The `shards` argument of `new` says
how much parallelism to support.

## API
All methods take `&self`. No call may panic, for any input. `ShardedMap<K, V>` can be shared by reference between
threads when `K` and `V` are `Send + Sync`.

### `ZeroShards`
```rust
pub struct ZeroShards;
```
- Derives `Debug, Clone, Copy, PartialEq, Eq`. Implements `Display` and `std::error::Error`.

### `ShardedMap::new`
```rust
pub fn new(shards: usize) -> Result<ShardedMap<K, V>, ZeroShards>
// where K: Hash + Eq
```
| Input | Returns |
|---|---|
| `shards == 0` | `Err(ZeroShards)` |
| `shards >= 1` | `Ok` with an empty map |

`shards` never changes what any method returns. Values such as 1, 2, 3, 7, 64 and 1000 must all work.

### `ShardedMap::insert`
```rust
pub fn insert(&self, key: K, value: V) -> Option<V>
```
Stores `value` under `key`. Returns the previous value for `key`, or `None` if there was none.

### `ShardedMap::get`
```rust
pub fn get(&self, key: &K) -> Option<V>
// where V: Clone
```
1. Returns a copy of the stored value, or `None` if `key` is absent.
2. While a caller holds the returned value, other threads can `insert`, `remove` and `update` the same key
   without waiting for the caller. The held copy does not change.
3. A `get` that runs while other threads replace the value for `key` returns a value one of them stored. It never
   returns `None` for a key that is not removed, and never a mix of two values.

### `ShardedMap::remove`
```rust
pub fn remove(&self, key: &K) -> Option<V>
```
Removes `key`. Returns its value, or `None` if `key` was absent.

### `ShardedMap::update`
```rust
pub fn update<R>(&self, key: K, default: impl FnOnce() -> V, f: impl FnOnce(&mut V) -> R) -> R
```
1. If `key` is absent, `default()` is called and its value is stored. If `key` is present, `default` is not
   called.
2. Then `f` runs on the stored value, and its result is returned.
3. The whole call is atomic for `key`. Concurrent `update` calls on one key never lose a write.

### `ShardedMap::len` and `ShardedMap::is_empty`
```rust
pub fn len(&self) -> usize
pub fn is_empty(&self) -> bool
```
1. `len` returns the number of entries. `is_empty` returns `len() == 0`.
2. Once all writers have finished, `len` is exact. While other threads write, the value may be out of date by the
   time it returns.

### Single-threaded behavior
For any sequence of `insert`, `remove`, `get` and `update` on one thread, every result equals what
`std::collections::HashMap` gives. For `update(key, || d, f)`, the `HashMap` equivalent is
`f(map.entry(key).or_insert(d))`.

## Example
`let m = ShardedMap::new(64).unwrap();` then these calls in order:

| Call | Returns |
|---|---|
| `m.insert("hits".to_string(), 1)` | `None` |
| `m.insert("hits".to_string(), 2)` | `Some(1)` |
| `m.update("hits".to_string(), \|\| 0, \|v\| { *v += 10; *v })` | `12` |
| `m.update("new".to_string(), \|\| 100, \|v\| { *v += 1; *v })` | `101` |
| `m.get(&"hits".to_string())` | `Some(12)` |
| `m.len()` | `2` |
| `m.remove(&"hits".to_string())` | `Some(12)` |
| `m.remove(&"hits".to_string())` | `None` |

Eight threads each call `update(i % 64, || 0u64, |v| *v += 1)` for `i` in `0..100_000`. Afterwards `len()` is 64 and
the 64 values sum to 800,000.

## Done when
Edit only `src/lib.rs`. Do not edit `tests/` or `Cargo.toml`. The starter compiles, and all 10 of its tests fail.

From `crates/ex07-sharded-map`, all of these exit 0:
```
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```
`cargo test` reports 10 passed and 1 ignored.

The ignored benchmark is a measurement, not a pass condition. Run it once on an idle machine with
`cargo test --release --test bench -- --ignored --nocapture`. It takes about 4 s and prints operations per second
for 1, 2, 4 and 8 threads on a 90% read, 10% write workload over 10,000 keys, for `ShardedMap` with 64 shards and
for a single `Mutex<HashMap<u64, u64>>`. Compare how each column changes from 1 to 8 threads.

## Extra tests (optional)
- `new(1)`: 4 threads insert 1,000 disjoint keys, then every key reads back.
- A key type whose `Hash` always returns the same value, with a correct `Eq`: `insert`, `get`, `remove` and
  `update` return the right values.
- `update` on an existing key with a `default` that increments a counter -> the counter stays 0.
- `fn is_sync<T: Sync>() {}; is_sync::<ShardedMap<String, u64>>();` compiles.
- `len()` called in a loop while 4 threads insert 10,000 distinct keys each -> never above 40,000, and exactly
  40,000 at the end.

## Stretch (optional)
- Make `get` take no lock, then rerun the benchmark and explain the 90/10 result.
- Add `retain(f)` that removes the entries for which `f` returns `false`.
- Add an `entry`-style API that lets the caller inspect a key, then insert or modify it atomically.
