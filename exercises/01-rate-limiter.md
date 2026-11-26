# Exercise 01: Token-bucket rate limiter

**Time:** ~2 h | **Area:** systems programming / networking | **Crate:** `crates/ex01-rate-limiter`
**Builds on:** none (standalone)

## Read first
- [Ch. 4 Ownership](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html)
- [Ch. 5 Structs](https://doc.rust-lang.org/book/ch05-00-structs.html)
- [Ch. 6 Enums](https://doc.rust-lang.org/book/ch06-00-enums.html)
- [Ch. 8 Collections](https://doc.rust-lang.org/book/ch08-00-common-collections.html)
- [Ch. 9 Error handling](https://doc.rust-lang.org/book/ch09-00-error-handling.html)
- [Ch. 10 Generics, traits](https://doc.rust-lang.org/book/ch10-00-generics.html) and [traits](https://doc.rust-lang.org/book/ch10-02-traits.html)
- std: [`HashMap`](https://doc.rust-lang.org/std/collections/struct.HashMap.html), [`u128`](https://doc.rust-lang.org/std/primitive.u128.html), [`Instant`](https://doc.rust-lang.org/std/time/struct.Instant.html)

## What you are building
A token bucket allows bursts up to a capacity and a sustained average rate. Proxies, API gateways and traffic
limiters use it to cap request or byte rates. You build the bucket with exact integer arithmetic and caller-supplied
time, so tests are deterministic and need no sleeps. Then you build a per-key limiter (for example per client IP)
whose memory use is bounded no matter how many distinct keys an attacker sends.

## Starting point
- Edit: `src/bucket.rs` (`TokenBucket`, `ConfigError`'s `Display`) and `src/keyed.rs` (`KeyedLimiter`).
  You may add private fields and helpers. Do not change public signatures.
- Do not edit: `src/clock.rs`, `src/lib.rs`, `tests/`.
- Run from `crates/ex01-rate-limiter`: `cargo test`. Add `--no-fail-fast` to see every test binary's result.
- The starter compiles. Every stub body is `todo!()`, so the given tests fail until you implement them.

## Requirements
Time is `u64` nanoseconds. `SEC` below means `1_000_000_000` ns.

**Clocks (provided)**
- R1. `Clock::now_ns(&self) -> u64`. `SystemClock` returns nanoseconds since its construction. `ManualClock`
  has `new(start_ns)`, `set(ns)` and `advance(ns)` (saturating); clones share one time source. The limiters take
  `now_ns` as an argument and never read a clock themselves.

**TokenBucket**
- R2. `TokenBucket::new(capacity: u32, refill_per_sec: u32) -> Result<TokenBucket, ConfigError>`.
  `capacity == 0` gives `ZeroCapacity`. Otherwise `refill_per_sec == 0` gives `ZeroRate`. The first check wins.
- R3. `ConfigError` has variants `ZeroCapacity`, `ZeroRate`, `ZeroMaxKeys`. It implements `Display` (a non-empty,
  distinct message per variant) and `std::error::Error`.
- R4. A new bucket holds `capacity` tokens.
- R5. `try_acquire(&mut self, now_ns: u64, n: u32) -> bool` takes `n` tokens and returns `true` only if at
  least `n` whole tokens are available. Otherwise it takes nothing and returns `false`.
- R6. `n == 0` returns `true`.
- R7. `n > capacity` returns `false` and leaves the bucket unchanged.
- R8. `available(&mut self, now_ns: u64) -> u32` returns the number of whole tokens after refilling to `now_ns`.
- R9. Refill adds exactly `elapsed_ns * refill_per_sec / SEC` tokens, where `elapsed_ns` is `now_ns` minus the
  largest `now_ns` seen so far. At rate 1, 999_999_999 ns adds no whole token and `SEC` adds exactly one.
- R10. The fractional part of a refill is kept. Splitting an interval across any number of calls yields the same
  token count as one call over the whole interval (below capacity).
- R11. Tokens never exceed `capacity`. After any idle time the bucket holds at most `capacity`.
- R12. A `now_ns` lower than the largest seen adds nothing, does not panic, and does not move the reference
  time backwards. Time already counted is not counted again when the clock catches up.
- R13. No panic or overflow for any `now_ns` (including `u64::MAX`) with `capacity` and `refill_per_sec` up to
  `u32::MAX`. Use `u128` for the product.
- R14. No floating-point types anywhere in `src/` (`f32`, `f64`).

**KeyedLimiter**
- R15. `KeyedLimiter<K: Hash + Eq + Clone>::new(capacity, refill_per_sec, max_keys: usize)` returns
  `Result<Self, ConfigError>`. Errors are checked in the order `ZeroCapacity`, `ZeroRate`, `ZeroMaxKeys`.
- R16. `check(&mut self, key: &K, now_ns: u64) -> Decision` takes one token from the key's bucket.
  `Allowed` if it succeeded, `Limited` if not. An unseen key gets a full bucket and this call takes its first token.
- R17. Keys have independent buckets and independent refill.
- R18. If the key is not in the table and the table already holds `max_keys` entries, `check` returns
  `Untracked`, stores nothing, and increments `untracked_count()` by 1. Callers treat `Untracked` as allowed.
- R19. Keys already in the table keep being limited when the table is full. Keys are never evicted.
- R20. `len()` never exceeds `max_keys`. `is_empty()` is `len() == 0`. `untracked_count()` returns `u64`.

## Given tests
35 tests in four files.

| Test (file) | Requirement |
|---|---|
| `manual_clock_set_and_advance`, `manual_clock_clones_share_time`, `system_clock_is_monotonic` (clock) | R1 |
| `bucket_driven_by_a_manual_clock` (clock) | R1, R9 |
| `rejects_zero_capacity`, `rejects_zero_rate`, `zero_capacity_wins_over_zero_rate` (bucket) | R2 |
| `errors_display_and_implement_error` (bucket) | R3 |
| `starts_full` (bucket) | R4 |
| `acquire_is_all_or_nothing` (bucket) | R5 |
| `zero_tokens_always_succeeds` (bucket) | R6 |
| `more_than_capacity_fails_and_changes_nothing`, `large_burst_of_whole_capacity` (bucket) | R7 |
| `exact_boundary_one_token_per_second` (bucket) | R9 (999_999_999 ns vs 1_000_000_000 ns) |
| `failed_acquire_does_not_lose_elapsed_time` (bucket) | R9, R10 |
| `fractional_carry_rate_three`, `fractional_remainder_accumulates_across_calls`, `small_steps_equal_one_big_step` (bucket) | R10 |
| `cap_after_long_idle` (bucket) | R11 |
| `clock_going_backwards_adds_nothing`, `backwards_step_does_not_double_count_time` (bucket) | R12 |
| `max_timestamp_does_not_panic_or_overflow`, `max_timestamp_small_bucket` (bucket) | R13 |
| `config_errors_in_order` (keyed) | R15 |
| `new_limiter_is_empty` (keyed) | R20 |
| `first_call_takes_a_token` (keyed) | R16 |
| `keys_are_isolated`, `keys_refill_independently`, `works_with_owned_string_keys` (keyed) | R16, R17 |
| `full_table_returns_untracked_and_stores_nothing` (keyed) | R18 |
| `tracked_keys_still_limited_when_table_is_full` (keyed) | R19 |
| `key_table_stays_bounded_under_one_million_distinct_keys` (keyed) | R18, R20 (`len() == 1000`, `untracked_count() == 999_000`) |
| `granted_tokens_bounded_by_capacity_plus_refill` (props, proptest) | tokens granted <= `capacity + floor(rate * (last - first) / SEC)` for any non-decreasing timestamps |
| `available_never_exceeds_capacity` (props, proptest) | R11, R13 |
| `splitting_an_interval_changes_nothing` (props, proptest) | R10 |

The proptest config uses a fixed seed, so runs are reproducible.

## Acceptance
From `crates/ex01-rate-limiter`:

1. `cargo test --no-fail-fast` exits 0 and reports 35 passed, 0 failed across `bucket` (19), `clock` (4),
   `keyed` (9) and `props` (3).
2. `cargo test --release` exits 0.
3. `cargo clippy --all-targets -- -D warnings` exits 0.
4. `cargo fmt --check` exits 0.
5. `grep -nE 'f32|f64' src/*.rs` prints nothing (R14).

On the unmodified starter, `cargo test --no-fail-fast` finishes in under a second with 32 failures and 3 passes.
The 3 passes are the `SystemClock`/`ManualClock` tests.

## Extra tests to write
Put them in `tests/extra.rs`.

1. IPv4-mapped IPv6 keys: use `std::net::IpAddr` as `K`. `::ffff:192.0.2.1` and `192.0.2.1` are one client, but
   `IpAddr` compares them as different keys. Write a test that expects them to share a bucket. It fails.
   Then write a `normalize(IpAddr) -> IpAddr` helper (see `Ipv6Addr::to_ipv4_mapped`) and call it before `check`.
   Catches: a client doubling its allowance by alternating address families.
2. Rate 3, capacity 3, drain at t=0, then call `available` at 1 ns steps up to 333_333_333 ns. Expect 0. At
   333_333_334 ns expect 1. Catches: rounding up instead of truncating.
3. `ManualClock` at `u64::MAX - 1`: drain, `advance(10)`, expect no panic and a full bucket at the end. Catches:
   `now - last` or `+` overflow in your own helper code.
4. Capacity `u32::MAX`, rate `u32::MAX`, drain it, then `available(u64::MAX)`. Expect `u32::MAX`. Catches: `u64`
   intermediate math or `as u32` truncation.
5. `try_acquire(t, 1)` repeated 10 times at the same `t` with 5 tokens: expect 5 `true` then 5 `false`.
   Catches: a failed call consuming tokens.
6. `max_keys = 1`, keys `a` then `b`, then `a` again. Expect `Allowed`, `Untracked`, `Limited` (capacity 1).
   Catches: `Untracked` evicting or replacing the stored key.
7. A failed `try_acquire` with `n > capacity` after a drain and a 0.5 s wait, followed by another 0.5 s wait:
   expect exactly one token. Catches: the early return skipping or double-applying refill.
8. Keyed limiter with `max_keys = 1000` and 10,000 distinct keys, then read `len()`. Expect 1000. Catches:
   checking the bound after insertion instead of before.

## Stretch (optional)
- Evict the least recently used key instead of returning `Untracked`. Decide what a newly evicted-in key's
  bucket starts with and write down why it cannot be exploited to bypass the limit. This leads into exercise 02.
- A thread-safe `&self` variant: `ConcurrentKeyedLimiter` with `check(&self, ...)`. Start with one `Mutex`,
  measure with 8 threads, then try sharding the table by key hash.
- Add `retry_after_ns(&mut self, now_ns, n) -> Option<u64>`: the exact wait until `n` tokens exist, or `None`
  if `n > capacity`. Test it against `try_acquire` at `now + wait - 1` and `now + wait`.
