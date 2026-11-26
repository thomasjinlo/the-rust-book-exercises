# Exercise 02: LRU cache

**Time:** 2.5 h | **Crate:** `crates/ex02-lru-cache`

## Read first
**Rust Book**
- [Ch. 4: Understanding Ownership](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html)
- [Ch. 8: Common Collections](https://doc.rust-lang.org/book/ch08-00-common-collections.html)
- [Ch. 9: Error Handling](https://doc.rust-lang.org/book/ch09-00-error-handling.html)
- [Ch. 10: Generic Types, Traits, and Lifetimes](https://doc.rust-lang.org/book/ch10-00-generics.html)
- [Ch. 11: Writing Automated Tests](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Ch. 13.2: Processing a Series of Items with Iterators](https://doc.rust-lang.org/book/ch13-02-iterators.html)
- [Ch. 15.3: Running Code on Cleanup with the Drop Trait](https://doc.rust-lang.org/book/ch15-03-drop.html)

**Standard library**
- [`Iterator`](https://doc.rust-lang.org/std/iter/trait.Iterator.html)

**Background**
- [LRU Cache Data Structure & Implementation (Interview Cake)](https://www.interviewcake.com/concept/java/lru-cache): how an LRU cache evicts, and how lookups and recency updates stay fast.
- [Cache replacement policies (Wikipedia)](https://en.wikipedia.org/wiki/Cache_replacement_policies): where LRU sits among eviction policies and what the others trade off.

## Task
Build a generic cache with a fixed capacity. When it is full and a new key arrives, it removes the entry that was
used longest ago and hands that entry back to the caller. The cache must also drop every value exactly once, keep
its memory bounded, and run 1,000,000 operations quickly.

## API
"Most recent" means most recently used. "Least recent" means least recently used. The cache owns its values and
returns them by value. `K: Hash + Eq + Clone`, and `V` can be any type. No call may panic, for any input.

### `ZeroCapacity`
```rust
pub struct ZeroCapacity;
```
- Derives `Debug, Clone, Copy, PartialEq, Eq`. Implements `Display` and `std::error::Error`.

### `LruCache::new`
```rust
pub fn new(capacity: usize) -> Result<LruCache<K, V>, ZeroCapacity>
```
| Input | Returns |
|---|---|
| `capacity == 0` | `Err(ZeroCapacity)` |
| `capacity >= 1` | `Ok` with an empty cache |

### `LruCache::put`
```rust
pub fn put(&mut self, key: K, value: V) -> Option<(K, V)>
```
1. If `key` is not in the cache and `len() < capacity()`, store the entry as most recent. Return `None`.
2. If `key` is not in the cache and `len() == capacity()`, store the entry as most recent and remove the least
   recent entry. Return the removed entry as `Some((key, value))`. The returned value is not dropped.
3. If `key` is in the cache, replace its value, make the entry most recent and return `None`. Nothing is removed,
   even when the cache is full. The old value is dropped before `put` returns.
4. A key that was removed by `remove` or returned by `put` counts as not in the cache.

### `LruCache::get` and `LruCache::get_mut`
```rust
pub fn get(&mut self, key: &K) -> Option<&V>
pub fn get_mut(&mut self, key: &K) -> Option<&mut V>
```
1. If `key` is in the cache, return its value and make the entry most recent.
2. Otherwise return `None` and change nothing.
3. A change made through `get_mut` is seen by later calls.

### `LruCache::peek`
```rust
pub fn peek(&self, key: &K) -> Option<&V>
```
Returns the value for `key`, or `None` if the key is not in the cache. The order does not change.

### `LruCache::remove`
```rust
pub fn remove(&mut self, key: &K) -> Option<V>
```
Returns `Some(value)` and takes the entry out, wherever it was in the order. If `key` is not in the cache,
returns `None` and changes nothing.

### `LruCache::len`, `capacity`, `is_empty`
```rust
pub fn len(&self) -> usize
pub fn capacity(&self) -> usize
pub fn is_empty(&self) -> bool
```
- `len()` is the number of entries. It never exceeds `capacity()`.
- `capacity()` is the value passed to `new`. It never changes.
- `is_empty()` is `len() == 0`.

### `LruCache::iter`
```rust
pub fn iter(&self) -> impl Iterator<Item = (&K, &V)>
```
Yields every entry once, from most recent to least recent. The order does not change. An empty cache yields
nothing.

### Drop and memory
- Every value passed to `put` is dropped exactly once: when it is replaced, when the caller drops it after `put` or
  `remove` returned it, or when the cache is dropped.
- Start with `LruCache<u64, u64>` of capacity 1,000 holding keys 0 to 999. Then run 1,000,000 times: `put(k, k)`
  with a new key `k`, then `remove(&k)`. The live heap bytes at the end are at most 3 times the live heap bytes
  after the first 1,000 puts.
- In a release build, 1,000,000 mixed `put`, `get` and `remove` calls on 4,096 distinct keys and capacity 1,024
  finish in under 5 seconds.

## Example
Capacity 3. "Order" lists keys from most to least recent.

| Call | Returns | Order after |
|---|---|---|
| `put(1, "a")`, `put(2, "b")`, `put(3, "c")` | `None` each time | 3, 2, 1 |
| `get(&1)` | `Some(&"a")` | 1, 3, 2 |
| `put(4, "d")` | `Some((2, "b"))` | 4, 1, 3 |
| `peek(&3)` | `Some(&"c")` | 4, 1, 3 |
| `remove(&1)` | `Some("a")` | 4, 3 |
| `put(5, "e")` | `None` | 5, 4, 3 |
| `get(&9)` | `None` | 5, 4, 3 |

## Done when
Edit only `src/lib.rs`. Do not edit `tests/` or the public signatures. The starter compiles, and all 19 of its tests
fail until you finish.

From `crates/ex02-lru-cache`, all of these exit 0:
```
cargo test
cargo test --release -- --ignored
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```
`cargo test` reports 19 passed (18 in `lru`, 1 in `memory`) and 1 ignored.
`cargo test --release -- --ignored` reports 1 passed.

## Extra tests (optional)
- Capacity 2: `put(1)`, `put(2)`, `get(&1)`, `put(3)`. Expect `Some` of the entry for key 2.
- Capacity 3: `put` key 1 a hundred times. Expect `None` every time and `len() == 1`.
- Fill to capacity, `remove` every key from least recent to most recent, fill again with new keys. Expect `iter`
  to list the new keys from newest to oldest.
- `String` keys and `Vec<u8>` values behave the same as integers.
- Full cache, `get` of a missing key, then `put` of a new key. Expect the entry that was least recent before the
  miss to be returned.

## Stretch (optional)
- `clear()`: remove every entry, drop every value once, keep `capacity()`.
- `get_or_insert_with(&mut self, K, impl FnOnce() -> V) -> &V`: on a miss, runs the closure once and inserts.
- Share one cache between 8 threads and compare one lock with several caches chosen by key.
