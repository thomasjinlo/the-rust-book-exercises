# Exercise 09: Fast log aggregation

**Time:** 3 h | **Crate:** `crates/ex09-fast-log-aggregation`

## Read first

**Rust Book**
- [Ch. 3.2: Data Types](https://doc.rust-lang.org/book/ch03-02-data-types.html)
- [Ch. 4.3: The Slice Type](https://doc.rust-lang.org/book/ch04-03-slices.html)
- [Ch. 8.2: Storing UTF-8 Encoded Text with Strings](https://doc.rust-lang.org/book/ch08-02-strings.html)
- [Ch. 10.3: Validating References with Lifetimes](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html)
- [Ch. 11.1: How to Write Tests](https://doc.rust-lang.org/book/ch11-01-writing-tests.html)
- [Ch. 11.3: Test Organization](https://doc.rust-lang.org/book/ch11-03-test-organization.html)
- [Ch. 13.2: Processing a Series of Items with Iterators](https://doc.rust-lang.org/book/ch13-02-iterators.html)
- [Ch. 13.4: Performance in Loops vs. Iterators](https://doc.rust-lang.org/book/ch13-04-performance.html)

**Standard library**
- [`slice`](https://doc.rust-lang.org/std/primitive.slice.html)

**Background**
- [Heap Allocations (The Rust Performance Book)](https://nnethercote.github.io/perf-book/heap-allocations.html): why each allocation costs time and which Rust operations allocate.
- [How to Parse Large Files with Zero-Copy Techniques in Rust (OneUptime)](https://oneuptime.com/blog/post/2026-01-25-parse-large-files-zero-copy-rust/view): reading a log buffer line by line and borrowing fields from it, including a final line without `\n`.

## Task
Build an `Aggregator` that reads a buffer of access-log lines and returns a `Summary`: counts of valid and malformed
lines, a count per status class, the total bytes and the slowest request. It must allocate nothing while it
aggregates, and in release mode it must run faster than the provided baseline.

## API
`Aggregator::aggregate` never panics, for any bytes. Offsets are in bytes from `input[0]`. Latencies are in
microseconds.

### `Aggregator`
```rust
pub fn new() -> Aggregator
```
`Aggregator` also implements `Default`. Allocating in `new` is allowed.

### `Aggregator::aggregate`
```rust
pub fn aggregate(&mut self, input: &[u8]) -> Summary
```
`input` is a sequence of lines. A line ends at `\n`. The last line may lack `\n`. Input that ends with `\n` has no
extra empty line after it. Each line is **valid** or **malformed**.

A valid line has these 7 fields, joined by exactly one space (0x20):

| Field | Valid values |
|---|---|
| `unix_ms` | 1 or more ASCII digits. Leading zeros allowed. Value at most `u64::MAX`. |
| `client_ip_v4` | 4 octets joined by `.`. Each octet is 1 to 3 digits, at most 255, with no leading zero unless it is exactly `0`. |
| `METHOD` | 1 or more characters from `A` to `Z`. |
| `path` | 1 or more bytes, none a space, the first one `/`. |
| `status` | Exactly 3 ASCII digits, value 100 to 599. |
| `bytes` | Same as `unix_ms`. |
| `latency_us` | Same as `unix_ms`. |

1. Any other line is malformed. This includes an empty line, a leading, trailing or doubled space, a signed number,
   a line with fewer or more than 7 fields, and a line ending in `\r` (`\r` is an ordinary byte).
2. Line length is not limited.
3. Each call returns the summary of its own `input` only, even on a reused `Aggregator`.
4. The call performs 0 allocations and 0 reallocations, also with malformed lines and with a 100,000-byte line.
   The test counts them with a global allocator around the call. Creating the `Aggregator` is not counted.
5. On any ASCII input the result equals `baseline::aggregate`.
6. In release mode on `datagen::generate(2_000_000, 2025)`, `aggregate` takes at most 1 / 1.4 of the time of
   `baseline::aggregate`. Each is timed as the best of 3 runs.

### `Summary`
```rust
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Summary {
    pub lines: u64,
    pub malformed: u64,
    pub by_status_class: [u64; 5],
    pub total_bytes: u64,
    pub slowest: Option<SlowRequest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlowRequest {
    pub latency_us: u64,
    pub line_offset: usize,
}
```
| Field | Meaning |
|---|---|
| `lines` | Number of valid lines. |
| `malformed` | Number of malformed lines. Every line counts in exactly one of `lines` and `malformed`. |
| `by_status_class` | Valid lines per status class. Index `k` counts status `(k+1)*100` to `(k+1)*100 + 99`, so index 0 is 1xx and index 4 is 5xx. |
| `total_bytes` | Sum of `bytes` over valid lines. Stops at `u64::MAX` instead of wrapping. |
| `slowest` | `None` if there is no valid line. Otherwise the largest `latency_us`, with `line_offset` the offset of the first byte of the first valid line that has it. |

Empty input gives `Summary::default()`.

## Provided (do not edit)
`baseline::aggregate(input: &str) -> Summary` is a simple implementation to compare against.
`datagen::generate(lines: usize, seed: u64) -> Vec<u8>` returns the same ASCII input for the same arguments, with
about 2% malformed lines. `Summary` and `SlowRequest` are defined in `src/lib.rs`.

## Example
Input with three valid lines and one malformed line. The last line has no `\n`.

```
1700000000000 10.0.0.9 GET /ok 200 512 40
1700000000001 10.0.0.9 GET /missing 404 0 900
garbage
1700000000003 10.0.0.2 POST /x 500 100 900
```

`Aggregator::new().aggregate(input)` returns:

| Field | Value | Why |
|---|---|---|
| `lines` | 3 | Lines 1, 2 and 4 are valid. |
| `malformed` | 1 | `garbage` has one field. |
| `by_status_class` | `[0, 1, 0, 1, 1]` | One 2xx, one 4xx, one 5xx. |
| `total_bytes` | 612 | 512 + 0 + 100. |
| `slowest` | `Some(SlowRequest { latency_us: 900, line_offset: 42 })` | Lines 2 and 4 tie. Line 2 starts after the 41 bytes of line 1 and its `\n`. |

These lines are malformed. `1 10.0.0.1 GET /a 200 5 5` is valid.

| Line | Why |
|---|---|
| `1 10.0.0.1 GET /a 99 5 5` | status has 2 digits |
| `1 10.0.0.1 GET /a 600 5 5` | status above 599 |
| `1 10.0.256.1 GET /a 200 5 5` | octet above 255 |
| `1 10.00.0.1 GET /a 200 5 5` | leading zero in octet |
| `1 10.0.0.1 GET /a 200 -5 5` | signed number |
| `1 10.0.0.1 GET /a 200 5 5 ` | trailing space |
| `1 10.0.0.1 GET /a 200 18446744073709551616 5` | `bytes` above `u64::MAX` |

Valid: `0007 0.0.0.1 X / 100 18446744073709551615 0000` (leading zeros, `u64::MAX`, path `/`).

## Done when
Edit only `src/aggregator.rs`. Do not edit `tests/` or the other files in `src/`. The starter compiles, and 11 of
its 12 tests fail until you finish.

From `crates/ex09-fast-log-aggregation`, all of these exit 0:
```
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo test --release -- --ignored --nocapture
```
`cargo test` reports 12 passed and 1 ignored. The last command runs the speed test, which is ignored by default
and meaningless in debug mode. It prints MB/s for both implementations and the ratio.

## Extra tests (optional)
- `\n\n\n` followed by one valid line -> `malformed == 3`, `lines == 1`.
- `1 10.0.0.1 GET /a 200 5 5\r\n` -> `malformed == 1`.
- Latency `18446744073709551615` -> `slowest.latency_us == u64::MAX`.
- `unix_ms` of `99999999999999999999` -> malformed.
- A generated buffer joined to itself -> every count doubles, and `slowest.line_offset` is in the first half.

## Stretch (optional)
- Add `Summary::merge(&mut self, other: &Summary)` and aggregate one buffer on several threads, splitting only at
  line boundaries. The result must equal the single-threaded `Summary`.
- Reach 8x the baseline in release mode.
- Aggregate input that arrives in 4 KB chunks, where a line can span two chunks, and match the whole-buffer result.
