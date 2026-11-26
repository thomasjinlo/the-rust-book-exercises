# Exercise 07: Sharded concurrent map

**Time:** 2 h | **Crate:** `crates/ex07-sharded-map`

## Task
Build a hash map that many threads use at once through a shared `&ShardedMap`. It splits its entries into
`shards` parts, each with its own lock, so threads that touch different keys do not wait for each other.

## Example
`let m = ShardedMap::new(64);`

| Call | Returns |
|---|---|
| `m.insert("hits", 1)` | `None` |
| `m.insert("hits", 2)` | `Some(1)` |
| `m.update("hits", \|\| 0, \|v\| { *v += 10; *v })` | `12` |
| `m.update("new", \|\| 100, \|v\| { *v += 1; *v })` | `101` |
| `m.get(&"hits")` | `Some(12)` |
| `m.get(&"other")` | `None` |

Eight threads each call `update(i % 64, || 0u64, |v| *v += 1)` for `i` in `0..100_000`. Afterwards the 64 values
sum to 800,000.

## API
`K: Hash + Eq`. All methods take `&self`, so the map can be shared between threads. Create the map with
`ShardedMap::new(shards: usize)`, where `shards >= 1`. The value of `shards` never changes what any method returns.

### `insert`
```rust
pub fn insert(&self, key: K, value: V) -> Option<V>
```
1. Stores `value` under `key`.
2. Returns the previous value for `key`, or `None` if there was none.

### `get`
```rust
pub fn get(&self, key: &K) -> Option<V>   // where V: Clone
```
1. Returns a copy of the stored value, or `None` if `key` is absent.
2. For any sequence of calls on one thread, results equal those of `std::collections::HashMap`.

### `update`
```rust
pub fn update<R>(&self, key: K, default: impl FnOnce() -> V, f: impl FnOnce(&mut V) -> R) -> R
```
1. If `key` is absent, `default()` is called and its value is stored. If `key` is present, `default` is not called.
2. Then `f` runs on the stored value, and its result is returned.
3. The whole call is atomic for `key`: concurrent `update` calls on one key never lose a write.

## Done when
Edit only `src/lib.rs`. From `crates/ex07-sharded-map`, `cargo test` reports 8 passed and 1 ignored (the starter:
8 failed, 1 ignored). Then run `cargo test --release -- --ignored`: 8 threads doing the same total number of
`update` calls on 1,024 keys with 64 shards must finish faster than 1 thread doing them all.

## Read first
- [Ch. 16.3: Shared-State Concurrency](https://doc.rust-lang.org/book/ch16-03-shared-state.html)
- [Ch. 8.3: Storing Keys with Associated Values in Hash Maps](https://doc.rust-lang.org/book/ch08-03-hash-maps.html)
- [Striped (Guava: Google Core Libraries for Java API)](https://guava.dev/releases/snapshot-jre/api/docs/com/google/common/util/concurrent/Striped.html)

## Stretch (optional)
- Add `remove(&self, key: &K) -> Option<V>` and `len(&self) -> usize`.
- Measure a 90% read, 10% write workload with a `RwLock` per shard and with a `Mutex` per shard.
