# Exercise 01: Rate limiter

**Time:** 2 h | **Crate:** `crates/ex01-rate-limiter`

## Read first
**Rust Book**
- [Ch. 3.2: Data Types](https://doc.rust-lang.org/book/ch03-02-data-types.html)
- [Ch. 4.2: References and Borrowing](https://doc.rust-lang.org/book/ch04-02-references-and-borrowing.html)
- [Ch. 5: Using Structs to Structure Related Data](https://doc.rust-lang.org/book/ch05-00-structs.html)
- [Ch. 6: Enums and Pattern Matching](https://doc.rust-lang.org/book/ch06-00-enums.html)
- [Ch. 8.3: Storing Keys with Associated Values in Hash Maps](https://doc.rust-lang.org/book/ch08-03-hash-maps.html)
- [Ch. 9.2: Recoverable Errors with Result](https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html)
- [Ch. 10.2: Defining Shared Behavior with Traits](https://doc.rust-lang.org/book/ch10-02-traits.html)
- [Ch. 11: Writing Automated Tests](https://doc.rust-lang.org/book/ch11-00-testing.html)

**Standard library**
- [`Error`](https://doc.rust-lang.org/std/error/trait.Error.html)
- [`Hash`](https://doc.rust-lang.org/std/hash/trait.Hash.html)

**Background**
- [Token bucket (Wikipedia)](https://en.wikipedia.org/wiki/Token_bucket): the burst size, the sustained rate and the allow/deny decision.
- [Scaling your API with rate limiters (Stripe)](https://stripe.com/blog/rate-limiters): why a service keeps one limiter per user.

## Task
Build a rate limiter that allows a short burst of requests and then a steady rate. Then build a second limiter
that keeps one such limit per key, such as per client address. The caller passes the time with every request.

## API
All times are `u64` nanoseconds. `SEC` is `1_000_000_000`. No call may panic, for any input.

### `ConfigError`
```rust
pub enum ConfigError { ZeroCapacity, ZeroRate, ZeroMaxKeys }
```
- Derives `Debug, Clone, Copy, PartialEq, Eq`. Implements `Display` and `std::error::Error`.
- Each variant's `Display` text is non-empty and different from the others.

### `TokenBucket::new`
```rust
pub fn new(capacity: u32, refill_per_sec: u32) -> Result<TokenBucket, ConfigError>
```
| Input | Returns |
|---|---|
| `capacity == 0` | `Err(ConfigError::ZeroCapacity)` |
| `capacity >= 1` and `refill_per_sec == 0` | `Err(ConfigError::ZeroRate)` |
| otherwise | `Ok` with a full bucket |

### `TokenBucket::try_acquire`
```rust
pub fn try_acquire(&mut self, now_ns: u64, n: u32) -> bool
```
Asks for `n` units at time `now_ns`. Returns `true` if allowed. The bucket follows these rules:

1. A new bucket holds `capacity` units.
2. Let `last` be the largest `now_ns` this bucket has seen. If `now_ns > last`, the bucket first gains
   `(now_ns - last) * refill_per_sec / SEC` units, up to at most `capacity`. A new bucket has no `last` and gains
   nothing on its first call.
3. If the bucket now holds at least `n` units, remove `n` and return `true`. Otherwise remove nothing and return
   `false`.

Units are exact. Fractions carry over between calls and are never rounded. A denied call still counts as a
call for rule 2.

### `KeyedLimiter::new`
```rust
pub fn new(capacity: u32, refill_per_sec: u32, max_keys: usize) -> Result<KeyedLimiter<K>, ConfigError>
// where K: Hash + Eq + Clone
```
| Input | Returns |
|---|---|
| `capacity == 0` | `Err(ConfigError::ZeroCapacity)` |
| `capacity >= 1` and `refill_per_sec == 0` | `Err(ConfigError::ZeroRate)` |
| both non-zero and `max_keys == 0` | `Err(ConfigError::ZeroMaxKeys)` |
| otherwise | `Ok` |

### `KeyedLimiter::check`
```rust
pub fn check(&mut self, key: &K, now_ns: u64) -> bool
```
- The first `max_keys` distinct keys passed to `check` are **tracked**. Each tracked key has its own bucket with
  the `capacity` and `refill_per_sec` given to `new`.
- For a tracked key, `check` returns what `try_acquire(now_ns, 1)` returns on that key's bucket.
- Any other key always gets `true`. It never becomes tracked and never replaces a tracked key.
- After `check` is called with 1,000,000 distinct keys and `max_keys = 1000`, the limiter uses under 4 MiB of
  heap.

## Provided (do not edit)
`Clock`, `SystemClock` and `ManualClock` in `src/clock.rs`. You may use them in your own tests. The limiters do
not use them.

## Example
`TokenBucket::new(2, 1)`:

| Call | Returns | Units held after |
|---|---|---|
| `try_acquire(0, 1)` | `true` | 1 |
| `try_acquire(0, 1)` | `true` | 0 |
| `try_acquire(0, 1)` | `false` | 0 |
| `try_acquire(SEC / 2, 1)` | `false` | 0.5 |
| `try_acquire(SEC, 1)` | `true` | 0 |
| `try_acquire(SEC * 9 / 10, 1)` | `false` | 0 (time went back) |
| `try_acquire(5 * SEC, 3)` | `false` | 2 (3 is more than the capacity) |
| `try_acquire(5 * SEC, 2)` | `true` | 0 |

`KeyedLimiter::new(1, 1, 2)`, all at time 0: `check(&"a")`, `check(&"b")`, `check(&"c")`, `check(&"a")`,
`check(&"c")` return `true`, `true`, `true`, `false`, `true`. Key `c` is not tracked.

`TokenBucket::new(10, 5)`, one request for 1 unit every `SEC / 20` from 0 up to `199 * SEC / 20`: 59 allowed,
141 denied.

## Done when
Edit only `src/bucket.rs` and `src/keyed.rs`. Do not edit `tests/` or the other files in `src/`. The starter
compiles, and 40 of its 43 tests fail until you finish.

From `crates/ex01-rate-limiter`, all of these exit 0:
```
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```
`cargo test` reports 43 passed.

## Extra tests (optional)
- `TokenBucket::new(1, 1)`, 180 requests at `k * SEC / 3`. Expect 60 allowed.
- `TokenBucket::new(1, 1)`: allowed at `10 * SEC`, then denied at `3 * SEC`, `10 * SEC` and `10 * SEC`.
- `TokenBucket::new(u32::MAX, u32::MAX)`: drain at 0, then `try_acquire(u64::MAX, u32::MAX)` returns `true`.
- `KeyedLimiter` with `String` keys built from a `&str` on every call gives the same results as `&str` keys.

## Stretch (optional)
- Evict the least recently used key instead of leaving extra keys unlimited.
- Add `retry_after_ns(&mut self, now_ns: u64, n: u32) -> Option<u64>`: how long until `n` units are available,
  or `None` if `n > capacity`.
