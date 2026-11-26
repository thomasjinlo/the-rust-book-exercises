# Exercise 01: Rate limiter

**Time:** 2 h | **Crate:** `crates/ex01-rate-limiter`

## Task
Build a token bucket: a rate limiter that allows a short burst of requests and then a steady rate. The caller
passes the time with every request.

## Example
`TokenBucket::new(2, 1)`. `SEC` is `1_000_000_000` ns.

| Call | Returns | Units held after |
|---|---|---|
| `try_acquire(0)` | `true` | 1 |
| `try_acquire(0)` | `true` | 0 |
| `try_acquire(0)` | `false` | 0 |
| `try_acquire(SEC / 2)` | `false` | 0.5 |
| `try_acquire(SEC)` | `true` | 0 |
| `try_acquire(SEC * 9 / 10)` | `false` | 0 (time went back) |
| `try_acquire(5 * SEC)` | `true` | 1 (the bucket holds at most 2) |
| `try_acquire(5 * SEC)` | `true` | 0 |
| `try_acquire(5 * SEC)` | `false` | 0 |

## API
All times are `u64` nanoseconds. Create the bucket with `TokenBucket::new(capacity: u32, refill_per_sec: u32)`,
where both are at least 1. A new bucket holds `capacity` units.

### `try_acquire`
```rust
pub fn try_acquire(&mut self, now_ns: u64) -> bool
```
1. Let `latest` be the largest `now_ns` the bucket has seen. If `now_ns > latest`, the bucket first gains
   `(now_ns - latest) * refill_per_sec / SEC` units, up to at most `capacity`. An earlier or equal time gains
   nothing.
2. If the bucket now holds at least 1 unit, it removes 1 and returns `true`. Otherwise it returns `false`.
3. Units are exact. Fractions carry over between calls and are never rounded.

## Done when
Edit only `src/bucket.rs`. From `crates/ex01-rate-limiter`, `cargo test` reports 8 passed. The starter compiles
and fails all 8. One test sends 200 requests to `TokenBucket::new(10, 5)`, one every `SEC / 20`, and exactly 59
must be allowed.

## Read first
- [Ch. 3.2: Data Types](https://doc.rust-lang.org/book/ch03-02-data-types.html)
- [Ch. 5.3: Methods](https://doc.rust-lang.org/book/ch05-03-method-syntax.html)
- [Token bucket (Wikipedia)](https://en.wikipedia.org/wiki/Token_bucket)

## Stretch (optional)
- Add a limiter that keeps one bucket per key, such as a client address.
- Let a request ask for `n` units at once.
