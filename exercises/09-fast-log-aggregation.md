# Exercise 09: Fast log aggregation

**Time:** 2 h | **Crate:** `crates/ex09-fast-log-aggregation`

## Task
Read a buffer of access-log lines and return a `Summary` of valid and malformed lines, server errors, total bytes
and the slowest request. Do it without a single heap allocation.

## Example
Input (the last line has no `\n`):

```
/ok 200 512 40
/missing 404 0 900
garbage
/x 500 100 700
```

`aggregate(input)` returns:

| Field | Value | Why |
|---|---|---|
| `lines` | 3 | Lines 1, 2 and 4 are valid. |
| `malformed` | 1 | `garbage` has one field. |
| `errors` | 1 | Only `/x 500` has a status from 500 to 599. |
| `total_bytes` | 612 | 512 + 0 + 100. |
| `max_latency_us` | 900 | The largest of 40, 900 and 700. |

## API
### `aggregate`
```rust
pub fn aggregate(input: &[u8]) -> Summary
```
`Summary` is defined in `src/lib.rs`. A valid line is `<path> <status> <bytes> <latency_us>`, joined by one space.
`path` is not empty and has no space. `status` is 3 ASCII digits from 100 to 599. `bytes` and `latency_us` are
ASCII digits that fit in a `u64`.

1. Lines end at `\n`. The last line may lack it, and a final `\n` does not start an extra empty line.
2. Every line counts as exactly one of `lines` and `malformed`. An empty line, or any line that does not match
   the format, is malformed.
3. `errors` counts valid lines with status 500 to 599. `total_bytes` is the sum of `bytes` over valid lines.
   `max_latency_us` is the largest `latency_us` over valid lines, or 0 if there is none.
4. Empty input gives `Summary::default()`. It never panics, for any bytes.

## Done when
Edit only `src/lib.rs`. From `crates/ex09-fast-log-aggregation`, `cargo test` reports 10 passed. At the start,
all 10 fail. One test runs `aggregate` on 100,000 generated lines and requires 0 allocations.

## Read first
- [Ch. 4.3: The Slice Type](https://doc.rust-lang.org/book/ch04-03-slices.html)
- [Ch. 13.2: Processing a Series of Items with Iterators](https://doc.rust-lang.org/book/ch13-02-iterators.html)
- [Heap Allocations (The Rust Performance Book)](https://nnethercote.github.io/perf-book/heap-allocations.html)

## Stretch (optional)
- Add a count per status code.
- Make `aggregate` faster than a version that builds a `String` for every line.
