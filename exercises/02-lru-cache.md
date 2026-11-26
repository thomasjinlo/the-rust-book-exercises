# Exercise 02: LRU cache

**Time:** 2 h | **Crate:** `crates/ex02-lru-cache`

## Task
Build a cache that holds at most `capacity` entries. When it is full and a new key arrives, it drops the entry
that was used longest ago.

## Example
`LruCache::new(2)`:

| Call | Returns | Cache after, newest first |
|---|---|---|
| `put(1, "a")` | | 1 |
| `put(2, "b")` | | 2, 1 |
| `get(&1)` | `Some(&"a")` | 1, 2 |
| `put(3, "c")` | | 3, 1 (2 is dropped) |
| `get(&2)` | `None` | 3, 1 |
| `put(1, "A")` | | 1, 3 |
| `get(&1)` | `Some(&"A")` | 1, 3 |

## API
`K: Hash + Eq + Clone`. `V` is any type. Create the cache with `LruCache::new(capacity: usize)`, where
`capacity >= 1`.

### `put`
```rust
pub fn put(&mut self, key: K, value: V)
```
1. Stores `value` under `key`. If `key` is already there, its value is replaced.
2. `key` becomes the newest entry.
3. If `key` was not there and the cache already held `capacity` entries, the oldest entry is dropped.

### `get`
```rust
pub fn get(&mut self, key: &K) -> Option<&V>
```
1. Returns the value for `key`, or `None` if it is not there.
2. On a hit, `key` becomes the newest entry. On a miss, nothing changes.

## Done when
Edit only `src/lib.rs`. From `crates/ex02-lru-cache`, `cargo test` reports 8 passed. One test runs 400,000 calls
on a cache of capacity 10,000 and must finish in under 5 seconds.

## Read first
- [Ch. 8.3: Storing Keys with Associated Values in Hash Maps](https://doc.rust-lang.org/book/ch08-03-hash-maps.html)
- [Ch. 10.1: Generic Data Types](https://doc.rust-lang.org/book/ch10-01-syntax.html)
- [LRU Cache Data Structure & Implementation (Interview Cake)](https://www.interviewcake.com/concept/java/lru-cache)

## Stretch (optional)
- Add `remove(&mut self, key: &K) -> Option<V>`.
- Add `peek(&self, key: &K) -> Option<&V>`, which does not change the order.
