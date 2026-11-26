# Exercise 09: Zero-allocation access-log aggregator

**Time:** ~3 h | **Area:** performance engineering | **Crate:** `crates/ex09-fast-log-aggregation`
**Builds on:** none (standalone)

## Read first
- Rust Book, slices: https://doc.rust-lang.org/book/ch04-03-slices.html
- Rust Book, strings: https://doc.rust-lang.org/book/ch08-02-strings.html
- Rust Book, lifetimes: https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html
- Rust Book, closures: https://doc.rust-lang.org/book/ch13-01-closures.html
- Rust Book, iterators: https://doc.rust-lang.org/book/ch13-02-iterators.html
- Rust Book, loops versus iterators performance: https://doc.rust-lang.org/book/ch13-04-performance.html
- Rust Book, writing tests: https://doc.rust-lang.org/book/ch11-01-writing-tests.html
- Rust Book, integration tests (why `zero_alloc.rs` is its own file): https://doc.rust-lang.org/book/ch11-03-test-organization.html
- std: [`GlobalAlloc`](https://doc.rust-lang.org/std/alloc/trait.GlobalAlloc.html)
- std: [`u64::from_str_radix`](https://doc.rust-lang.org/std/primitive.u64.html#method.from_str_radix) and [`str::parse`](https://doc.rust-lang.org/std/primitive.str.html#method.parse) (what the baseline uses; you will write your own digit loop)
- The Rust Performance Book, heap allocations: https://nnethercote.github.io/perf-book/heap-allocations.html

## What you are building
Log pipelines and metrics agents scan gigabytes of access logs per minute. At that volume, per-line heap allocation and generic text APIs dominate the cost. You build an aggregator that takes one complete byte buffer of log lines, parses each line in place, and returns counters: valid and malformed lines, a count per status class, total bytes, and the slowest request. It allocates nothing while aggregating. A counting global allocator in the tests proves that, and a throughput test compares it with a deliberately naive baseline.

## Concepts
**Why allocation costs time.** `String::from(x)` asks the allocator for a block, copies the bytes, and later asks it to free the block. The allocator searches free lists, may take a lock, and touches memory that is cold in the CPU cache. One call costs tens of nanoseconds. A line with 7 fields parsed into 7 `String`s makes 7 such calls. At 2,000,000 lines that is 14,000,000 calls before any real work happens.

**Borrow slices instead.** A `&[u8]` is a pointer and a length into memory that already exists. Cutting a field out of the input copies nothing and allocates nothing. The input outlives every slice cut from it, so the borrow checker is satisfied without extra work.

**Worked example by hand.** Input line (bytes of one buffer, no `\n` shown):

```
1700000000000 10.0.0.9 GET /ok 200 1 1
```

| Step | Remaining slice before | Action | Result |
|---|---|---|---|
| 1 | `1700000000000 10.0.0.9 GET ...` | read digits up to the first space | `unix_ms = 1700000000000`, slice moves past the space |
| 2 | `10.0.0.9 GET /ok 200 1 1` | read 4 octets, check each 1 to 3 digits, no leading zero, at most 255 | IP valid |
| 3 | `GET /ok 200 1 1` | take `A`-`Z` bytes up to a space | method `GET` |
| 4 | `/ok 200 1 1` | check first byte `/`, skip to next space | path ok |
| 5 | `200 1 1` | exactly 3 digits then a space | `status = 200`, class index `200 / 100 - 1 = 1` |
| 6 | `1 1` | digits, space | `bytes = 1` |
| 7 | `1` | digits, then end of line | `latency = 1` |

The result adds 1 to `lines`, 1 to `by_status_class[1]`, 1 to `total_bytes`, and updates `slowest`. If any step fails, the line adds 1 to `malformed` and nothing else. No step created a `String` or a `Vec`.

**Counting allocator.** Rust lets a program replace its allocator with any type that implements `GlobalAlloc`. A counting allocator forwards every call to the system allocator and bumps a counter first. A test reads the counter before and after the code under test; equal values mean zero allocations.

```rust
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}
#[global_allocator]
static A: Counting = Counting;
```

The given test counts per thread instead of globally, because the test harness runs other tests on other threads. It lives in its own file because `#[global_allocator]` applies to the whole test binary.

## Starting point
- Edit only `src/aggregator.rs`. It contains stubs that panic with `todo!()`.
- Do not edit `src/lib.rs` (types and the line-format definition), `src/baseline.rs` (naive reference), `src/datagen.rs` (deterministic generator), or anything under `tests/`.
- `Cargo.toml` already lists `memchr`. Only the stretch goal uses it.
- Run: `cargo test` in the crate directory.

## Requirements
Line format: `<unix_ms> <client_ip_v4> <METHOD> <path> <status> <bytes> <latency_us>`, fields separated by exactly one space, lines ended by `\n`. `lib.rs` is the authority; the items below restate it.

**Validity**

R1. A line is valid only if it has exactly 7 fields, none empty. Two adjacent spaces, a leading space, or a trailing space make it malformed.
R2. `unix_ms`, `bytes` and `latency_us` are ASCII digits only (no `-`, no `+`) and fit in `u64`. Leading zeros are allowed. A value above `u64::MAX` is malformed.
R3. `client_ip_v4` has four octets separated by `.`. Each octet has 1 to 3 digits, is at most 255, and has no leading zero unless it is exactly `0`.
R4. `METHOD` is one or more characters from `A` to `Z`. `path` starts with `/`.
R5. `status` is exactly 3 digits with value 100 to 599. `99`, `099`, `600` and `1000` are malformed.
R6. An empty line is malformed. `\r` is an ordinary byte, so a line ending in `\r\n` is malformed. The final line may lack `\n`. Input that ends with `\n` has no extra empty line after it. There is no limit on line length.

**Summary**

R7. `lines` counts valid lines. `malformed` counts malformed lines. Each input line adds to exactly one of them.
R8. `by_status_class[k]` counts valid lines with status in `(k+1)*100 ..= (k+1)*100 + 99`.
R9. `total_bytes` is the sum of `bytes` over valid lines, saturating at `u64::MAX`.
R10. `slowest` is `None` without valid lines. Otherwise it holds the maximum `latency_us` and `line_offset`, the byte offset from `input[0]` of the first byte of the first line with that latency.
R11. Each `aggregate` call is independent: it returns the summary of its own `input` only.

**Equality and memory**

R12. `Aggregator::aggregate` returns the same `Summary` as `baseline::aggregate` on any ASCII input.
R13. `aggregate` performs 0 allocations and 0 reallocations, including when lines are malformed or very long.

**Speed**

R14. In release mode on 2,000,000 generated lines, `aggregate` is at least 1.4 times faster than `baseline::aggregate` (see Acceptance).

## Milestones
Run the filter after each stage. Tests not named in a stage may still fail.

| # | Do | Requirements | Tests that go green | Run |
|---|---|---|---|---|
| 1 | Split the buffer into lines and count empty ones as malformed. | R6 (empty lines, final `\n`) | `empty_input`, `line_splitting` | `cargo test --test small -- empty_input line_splitting` |
| 2 | Parse one line into fields with slices and a digit loop. Count lines as valid or malformed. | R1 to R5, R7 | `each_malformed_variant_is_counted_and_skipped`, `accepted_edge_values` | `cargo test --test small -- malformed accepted` |
| 3 | Fill the rest of the `Summary`. | R8 to R11 | `status_classes_and_saturating_bytes`, `slowest_tie_keeps_lowest_offset`, `hand_written_valid_lines` | `cargo test --test small` |
| 4 | Compare with the baseline on generated and corrupted input. | R12 | `generated_buffer_matches_baseline`, `mutated_input_matches_baseline` | `cargo test --test equality` |
| 5 | Remove every allocation. | R13 | `aggregate_allocates_nothing`, `malformed_and_oversized_lines_allocate_nothing` | `cargo test --test zero_alloc` |
| 6 | Measure. | R14 | `aggregator_beats_baseline_throughput` | `cargo test --release -- --ignored --nocapture` |

If your first version uses `str::split` and `collect::<Vec<_>>()`, stage 5 fails. Replace the `Vec` with slice cursors.

## Common mistakes
- Parsing with `str::parse::<u64>()` after `from_utf8` accepts a leading `+`. Reject non-digits yourself.
- Treating the last field like the others. A trailing space after `latency_us` must make the line malformed.
- Counting the empty piece after a final `\n` as a line.
- Accumulating digits with `v * 10 + d` on a `u64`. A 20-digit field overflows; use `checked_mul` and `checked_add`.
- Using `>=` when tracking `slowest`. The first line at the maximum latency must win.
- Computing `line_offset` from line numbers instead of bytes, or forgetting that a malformed line still advances the offset.

## Given tests
12 tests run by default and 1 is `#[ignore]`.

| File | Test | Requirement |
|---|---|---|
| `tests/small.rs` | `empty_input` | R6, R7 |
| `tests/small.rs` | `line_splitting` | R6 |
| `tests/small.rs` | `each_malformed_variant_is_counted_and_skipped` | R1 to R6 (23 variants, each checked terminated and, if non-empty, unterminated) |
| `tests/small.rs` | `accepted_edge_values` | R2, R3, R5 |
| `tests/small.rs` | `status_classes_and_saturating_bytes` | R8, R9 |
| `tests/small.rs` | `slowest_tie_keeps_lowest_offset` | R10 |
| `tests/small.rs` | `hand_written_valid_lines` | R7 to R11 (exact `Summary`, unterminated last line, second call) |
| `tests/equality.rs` | `generated_buffer_matches_baseline` | R12 (100,000 generated lines) |
| `tests/equality.rs` | `mutated_input_matches_baseline` | R1 to R6, R12 (300 randomly corrupted buffers, fixed seed) |
| `tests/zero_alloc.rs` | `aggregate_allocates_nothing` | R13 (1,000,000 lines) |
| `tests/zero_alloc.rs` | `malformed_and_oversized_lines_allocate_nothing` | R13 (100,000-byte line) |
| `tests/zero_alloc.rs` | `counting_allocator_works` | Guards the counter against reading 0 for the wrong reason. It passes on the unmodified starter. |
| `tests/perf.rs` | `aggregator_beats_baseline_throughput` (`#[ignore]`) | R14 |

## Acceptance
1. `cargo test` exits 0 with 12 tests passed in total across test binaries. On the unmodified starter, 11 of the 12 fail and `counting_allocator_works` passes. Use `cargo test --no-fail-fast` to see every binary run.
2. `cargo clippy --all-targets -- -D warnings` exits 0.
3. Speed: `cargo test --release -- --ignored --nocapture`. The test generates 2,000,000 lines (about 115 MB), takes the best of 3 runs of each implementation, prints MB/s for both and the ratio, and asserts `ratio >= 1.4`. The reference solution measured 4.4x on the verifying machine (baseline about 91 MB/s, aggregator about 400 MB/s), more than 3 times the threshold, so a loaded host still passes. Record your own ratio in your notes. Do not run this test in debug mode; the ratio is meaningless there.

## Extra tests to write
1. `\n\n\n` plus one valid line: `malformed == 3`, `lines == 1`. Catches skipping empty lines.
2. A valid line followed by `\n` and then nothing, versus the same line with no `\n`: equal summaries. Catches a final-line off-by-one.
3. `1 10.0.0.1 GET /a 200 5 5` with the `\n` replaced by `\r\n`: `malformed == 1`. Catches trimming `\r`.
4. Status `200` on a line with latency `18446744073709551615`: `slowest.latency_us == u64::MAX`. Catches latency stored in a narrower type.
5. 20-digit `unix_ms` equal to `99999999999999999999`: malformed. Catches wrapping overflow.
6. Non-ASCII bytes (`0xFF`) in the path field: valid if the other fields are valid, because `aggregate` takes `&[u8]`. Catches an accidental `from_utf8` call. The baseline cannot take this input, so assert the exact `Summary`.
7. A generated buffer concatenated with itself: every count doubles and `slowest.line_offset` stays in the first half. Catches state leaking across lines.
8. Property: for random valid-line sets built by your own generator, `lines` equals the number generated and `total_bytes` equals their sum. Catches a parser that is correct only on `datagen`'s output.

## Stretch (optional)
1. Replace the newline search with `memchr::memchr` and report the ratio change.
2. Add `Summary::merge(&mut self, other: &Summary)` and aggregate in parallel: split the buffer at newline boundaries into one part per core, run one `Aggregator` per thread, merge. Decide how `slowest.line_offset` is rebased.
3. Parse fields while scanning for the newline so each byte is read once, and report the ratio change.
