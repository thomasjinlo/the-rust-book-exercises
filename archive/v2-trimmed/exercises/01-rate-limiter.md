# Exercise 01: Token-bucket rate limiter

**Time:** ~2 h | **Area:** systems programming / networking | **Crate:** `crates/ex01-rate-limiter`
**Builds on:** none (standalone)

## Read first
- [Ch. 4 Ownership](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html)
- [Ch. 5 Structs](https://doc.rust-lang.org/book/ch05-00-structs.html)
- [Ch. 6 Enums](https://doc.rust-lang.org/book/ch06-00-enums.html)
- [Ch. 8 Collections](https://doc.rust-lang.org/book/ch08-00-common-collections.html)
- [Ch. 9 Error handling](https://doc.rust-lang.org/book/ch09-00-error-handling.html)
- [Ch. 10 Generics](https://doc.rust-lang.org/book/ch10-00-generics.html)
- std: [`HashMap`](https://doc.rust-lang.org/std/collections/struct.HashMap.html), [`u128`](https://doc.rust-lang.org/std/primitive.u128.html)

## What you are building
A token bucket allows bursts up to a capacity and a sustained average rate. Proxies, API gateways and traffic
limiters use it to cap request or byte rates. You build the bucket with exact integer arithmetic and caller-supplied
time, so tests are deterministic and need no sleeps. Then you build a per-key limiter (for example per client IP)
whose memory use stays bounded no matter how many distinct keys a client sends.

## Concepts
A bucket holds up to `capacity` tokens. Each request removes tokens. Time adds `refill_per_sec` tokens per second,
never above `capacity`. A request succeeds only if enough whole tokens are present.

```
capacity 5, rate 2/s, bucket drained to 0 at t = 0

t (ms)    elapsed    tokens added = elapsed_ns * 2 / 1e9    whole tokens
  250     250e6 ns   0.5                                      0
  500     500e6 ns   1.0                                      1
 1000     1e9 ns     2.0                                      2
10000     10e9 ns    20.0, capped                             5
```

Whole tokens only, but fractions must survive between calls. Count in units of 1/1e9 token:
`level += elapsed_ns * rate`. A whole token is `1e9` units. One call at 250 ms adds `250e6 * 2 = 500e6` units
(0.5 token). A second call at 500 ms adds another `500e6`, so the level is `1e9` units: exactly one token.
If you rounded down to whole tokens after the first call, you would lose the 0.5 and end with zero.

Time is passed in by the caller as `now_ns` (`u64` nanoseconds). The bucket remembers the largest `now_ns` it has
seen. A smaller value adds nothing and does not move that mark back. Otherwise a clock that jumps back and
forward would refill the same interval twice.

`elapsed_ns * rate` can reach about `2^64 * 2^32`. That overflows `u64`, so compute it in `u128`.

The per-key limiter is a `HashMap<K, TokenBucket>`. An attacker can send endless distinct keys, so the table has
a hard size limit. When it is full, a key that is not in the table is allowed and not stored. Keys already
in the table stay limited.

## Milestones
Run the commands from `crates/ex01-rate-limiter`. The 3 clock tests that need no bucket pass on the starter.

1. **Errors.** R2, R3 (and R1, provided). Green: `rejects_zero_capacity`, `rejects_zero_rate`,
   `zero_capacity_wins_over_zero_rate`, `errors_display_and_implement_error`.
   Run: `cargo test --test bucket -- rejects zero_capacity errors_display`.
2. **Whole tokens, no refill.** R4, R5, R6. Green: `starts_full`, `acquire_is_all_or_nothing`,
   `zero_tokens_always_succeeds`, `more_than_capacity_fails_and_changes_nothing`, `large_burst_of_whole_capacity`.
   Run: `cargo test --test bucket -- starts_full all_or_nothing zero_tokens more_than_capacity large_burst`.
3. **Refill.** R7, R8, R9. Green: `exact_boundary_one_token_per_second`, `failed_acquire_does_not_lose_elapsed_time`,
   `fractional_carry_rate_three`, `fractional_remainder_accumulates_across_calls`, `small_steps_equal_one_big_step`,
   `cap_after_long_idle`, `bucket_driven_by_a_manual_clock`.
   Run: `cargo test --test bucket --test clock`.
4. **Hostile time.** R10, R11, R12. Green: `clock_going_backwards_adds_nothing`,
   `backwards_step_does_not_double_count_time`, `max_timestamp_does_not_panic_or_overflow`,
   `max_timestamp_small_bucket`, and the 3 tests in `tests/random.rs`.
   Run: `cargo test --test bucket --test random`. The whole `bucket` file (19 tests) should now pass.
5. **Keyed limiter.** R13 to R16. Green: all 9 tests in `tests/keyed.rs`.
   Run: `cargo test --test keyed`.
6. **Final.** Run the commands under Acceptance.

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
  least `n` whole tokens are available. Otherwise it takes nothing and returns `false`. `n == 0` returns `true`.
  `n > capacity` returns `false` and leaves the bucket unchanged.
- R6. `available(&mut self, now_ns: u64) -> u32` returns the number of whole tokens after refilling to `now_ns`.
- R7. Refill adds exactly `elapsed_ns * refill_per_sec / SEC` tokens, where `elapsed_ns` is `now_ns` minus the
  largest `now_ns` seen so far. At rate 1, 999_999_999 ns adds no whole token and `SEC` adds exactly one.
  A failed `try_acquire` still counts the elapsed time.
- R8. The fractional part of a refill is kept. Splitting an interval across any number of calls yields the same
  token count as one call over the whole interval (below capacity).
- R9. Tokens never exceed `capacity`. After any idle time the bucket holds at most `capacity`.
- R10. A `now_ns` lower than the largest seen adds nothing, does not panic, and does not move the reference
  time backwards. Time already counted is not counted again when the clock catches up.
- R11. No panic or overflow for any `now_ns` (including `u64::MAX`) with `capacity` and `refill_per_sec` up to
  `u32::MAX`. Use `u128` for the product.
- R12. No floating-point types anywhere in `src/` (`f32`, `f64`).

**KeyedLimiter**
- R13. `KeyedLimiter<K: Hash + Eq + Clone>::new(capacity, refill_per_sec, max_keys: usize)` returns
  `Result<Self, ConfigError>`. Errors are checked in the order `ZeroCapacity`, `ZeroRate`, `ZeroMaxKeys`.
- R14. `check(&mut self, key: &K, now_ns: u64) -> bool` takes one token from the key's bucket and returns
  whether it succeeded. An unseen key gets a full bucket and this call takes its first token.
- R15. Keys have independent buckets and independent refill. Keys already in the table keep being limited when
  the table is full. Keys are never evicted.
- R16. If the key is not in the table and the table already holds `max_keys` entries, `check` returns `true`
  and stores nothing. `len()` never exceeds `max_keys`. `is_empty()` is `len() == 0`.

## Common mistakes
- Converting to whole tokens after every call. The fraction is lost and a slow rate never refills (R8).
- Setting the reference time only when a request succeeds. Time between failed calls is then counted twice or
  lost (R7).
- Assigning `last = now_ns` unconditionally. A backwards step then moves the mark back and refills twice (R10).
- Multiplying `elapsed_ns * rate` in `u64`. It overflows for long gaps with high rates. Use `u128` (R11).
- Removing tokens before checking that `n` fits. A failed call must leave the bucket unchanged (R5).
- Checking the table size after inserting a new key. The table then grows to `max_keys + 1` (R16).

## Starting point
- Edit: `src/bucket.rs` (`TokenBucket`, `ConfigError`'s `Display`) and `src/keyed.rs` (`KeyedLimiter`).
  You may add private fields and helpers. Do not change public signatures.
- Do not edit: `src/clock.rs`, `src/lib.rs`, `tests/`.
- Run from `crates/ex01-rate-limiter`: `cargo test --no-fail-fast` shows every test binary's result.
- The starter compiles. Every stub body is `todo!()`, so the given tests fail until you implement them.

## Given tests
35 tests in four files. `tests/random.rs` uses a fixed-seed PRNG written in the file, so runs are reproducible.

| Test (file) | Requirement |
|---|---|
| `manual_clock_set_and_advance`, `manual_clock_clones_share_time`, `system_clock_is_monotonic` (clock) | R1 |
| `bucket_driven_by_a_manual_clock` (clock) | R1, R7 |
| `rejects_zero_capacity`, `rejects_zero_rate`, `zero_capacity_wins_over_zero_rate` (bucket) | R2 |
| `errors_display_and_implement_error` (bucket) | R3 |
| `starts_full` (bucket) | R4, R6 |
| `acquire_is_all_or_nothing`, `zero_tokens_always_succeeds`, `more_than_capacity_fails_and_changes_nothing`, `large_burst_of_whole_capacity` (bucket) | R5 |
| `exact_boundary_one_token_per_second`, `failed_acquire_does_not_lose_elapsed_time` (bucket) | R7 |
| `fractional_carry_rate_three`, `fractional_remainder_accumulates_across_calls`, `small_steps_equal_one_big_step` (bucket) | R8 |
| `cap_after_long_idle` (bucket) | R9 |
| `clock_going_backwards_adds_nothing`, `backwards_step_does_not_double_count_time` (bucket) | R10 |
| `max_timestamp_does_not_panic_or_overflow`, `max_timestamp_small_bucket` (bucket) | R11 |
| `config_errors_in_order` (keyed) | R13 |
| `new_limiter_is_empty` (keyed) | R16 |
| `first_call_takes_a_token` (keyed) | R14 |
| `keys_are_isolated`, `keys_refill_independently`, `works_with_owned_string_keys` (keyed) | R14, R15 |
| `tracked_keys_still_limited_when_table_is_full` (keyed) | R15 |
| `full_table_allows_new_key_without_storing_it` (keyed) | R16 |
| `key_table_stays_bounded_under_one_million_distinct_keys` (keyed) | R16 (`len() == 1000` after 1,000,000 keys) |
| `granted_tokens_bounded_by_capacity_plus_refill` (random) | R7, R9: tokens granted <= `capacity + floor(rate * now / SEC)` |
| `available_never_exceeds_capacity` (random) | R9, R11 |
| `splitting_an_interval_changes_nothing` (random) | R8 |

## Acceptance
From `crates/ex01-rate-limiter`:

1. `cargo test --no-fail-fast` exits 0 and reports 35 passed, 0 failed across `bucket` (19), `clock` (4),
   `keyed` (9) and `random` (3).
2. `cargo test --release` exits 0.
3. `cargo clippy --all-targets -- -D warnings` exits 0.
4. `cargo fmt --check` exits 0.
5. `grep -nE 'f32|f64' src/*.rs` prints nothing (R12).

On the unmodified starter, `cargo test --no-fail-fast` finishes in under a second with 32 failures and 3 passes.
The 3 passes are the `SystemClock`/`ManualClock` tests.

## Extra tests to write
Put them in `tests/extra.rs`.

1. Rate 3, capacity 3, drain at t=0, then call `available` at 333_333_333 ns. Expect 0. At 333_333_334 ns
   expect 1. Catches: rounding up instead of truncating.
2. `ManualClock` at `u64::MAX - 1`: drain, `advance(10)`, expect no panic and a full bucket at the end. Catches:
   `now - last` or `+` overflow in your own helper code.
3. Capacity `u32::MAX`, rate `u32::MAX`, drain it, then `available(u64::MAX)`. Expect `u32::MAX`. Catches: `u64`
   intermediate math or `as u32` truncation.
4. `try_acquire(t, 1)` repeated 10 times at the same `t` with 5 tokens: expect 5 `true` then 5 `false`.
   Catches: a failed call consuming tokens.
5. `max_keys = 1`, keys `a` then `b`, then `a` again, capacity 1. Expect `true`, `true`, `false`. Catches: a
   full table evicting or replacing the stored key.
6. A failed `try_acquire` with `n > capacity` after a drain and a 0.5 s wait, followed by another 0.5 s wait:
   expect exactly one token at rate 1. Catches: the early return skipping or double-applying refill.
7. Keyed limiter with `max_keys = 1000` and 10,000 distinct keys, then read `len()`. Expect 1000. Catches:
   checking the bound after insertion instead of before.

## Stretch (optional)
- Evict the least recently used key instead of allowing unstored keys. Decide what an evicted-in key's bucket
  starts with and write down why that cannot be used to bypass the limit. This leads into exercise 02.
- A thread-safe `&self` variant with `check(&self, ...)`. Start with one `Mutex`, measure with 8 threads, then
  try sharding the table by key hash.
- Add `retry_after_ns(&mut self, now_ns, n) -> Option<u64>`: the exact wait until `n` tokens exist, or `None`
  if `n > capacity`. Test it against `try_acquire` at `now + wait - 1` and `now + wait`.
