# Exercise 13: Zero-allocation access-log aggregator

**Time:** ~4 h | **Area:** performance engineering | **Crate:** `crates/ex13-fast-log-aggregation`
**Builds on:** none (standalone)

## Read first
- Rust Book, iterators: https://doc.rust-lang.org/book/ch13-02-iterators.html
- Rust Book, strings and slices: https://doc.rust-lang.org/book/ch08-02-strings.html
- Rust Book, lifetimes: https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html
- Rust Book, unsafe (you do not need it here): https://doc.rust-lang.org/book/ch20-01-unsafe-rust.html
- The Rust Performance Book: [Benchmarking](https://nnethercote.github.io/perf-book/benchmarking.html), [Profiling](https://nnethercote.github.io/perf-book/profiling.html), [Heap Allocations](https://nnethercote.github.io/perf-book/heap-allocations.html), [Hashing](https://nnethercote.github.io/perf-book/hashing.html), [I/O](https://nnethercote.github.io/perf-book/io.html)
- `GlobalAlloc`: https://doc.rust-lang.org/std/alloc/trait.GlobalAlloc.html
- `FromStr` (what the baseline's `parse` uses): https://doc.rust-lang.org/std/str/trait.FromStr.html
- Slices (`split`, `chunks`): https://doc.rust-lang.org/std/primitive.slice.html
- `Iterator::position`: https://doc.rust-lang.org/std/iter/trait.Iterator.html
- `memchr` (SIMD byte search, optional): https://docs.rs/memchr/latest/memchr/
- `criterion` (benchmark harness, optional, not used by the tests): https://docs.rs/criterion/latest/criterion/

## What you are building
Log pipelines and metrics agents scan gigabytes of access logs per minute. At that volume, per-line heap allocation and generic text APIs dominate the cost. You build a streaming aggregator that reads raw bytes in arbitrary chunks, parses each line in place, and updates counters, a latency histogram, and an exact distinct-client set held in one fixed-size table. After `Aggregator::new()` it never touches the heap. A counting global allocator in the tests proves that, and a throughput test compares it with a deliberately naive baseline.

## Starting point
- Edit only `src/aggregator.rs`. It contains stubs that panic with `todo!()`.
- Do not edit `src/lib.rs` (types, constants, line-format definition), `src/baseline.rs` (naive reference) or `src/datagen.rs` (deterministic generator), or anything under `tests/`.
- `Cargo.toml` already lists `memchr`. You may use it or not. The release profile keeps line tables (`debug = 1`) so profilers show function names.
- Run: `cargo test` in the crate directory.

## Requirements
Line format: `<unix_ms> <client_ip_v4> <METHOD> <path> <status> <bytes> <latency_us>`, fields separated by exactly one space, lines ended by `\n`. `lib.rs` is the authority; the numbered items below restate it.

**Validity**

R1. A line is valid only if it has exactly 7 fields, none empty. Two adjacent spaces, a leading space, or a trailing space make it malformed.
R2. `unix_ms`, `bytes` and `latency_us` are ASCII digits only (no `-`, no `+`) and fit in `u64`. Leading zeros are allowed. A value above `u64::MAX` is malformed.
R3. `client_ip_v4` has four octets separated by `.`. Each octet has 1 to 3 digits, is at most 255, and has no leading zero unless it is exactly `0`.
R4. `METHOD` is one or more characters from `A` to `Z`. `path` starts with `/`.
R5. `status` is exactly 3 digits with value 100 to 599. `99`, `099`, `600` and `1000` are malformed.
R6. An empty line is malformed. `\r` is an ordinary byte, so a line ending in `\r\n` is malformed. The final line may lack `\n`. Input that ends with `\n` has no extra empty line after it.
R7. A line with more than `CARRY_CAP` (4096) content bytes (not counting `\n`) is malformed. A line of exactly 4096 bytes is valid if it parses.

**Summary**

R8. `lines` counts valid lines. `malformed` counts malformed lines. Each input line adds to exactly one of them.
R9. `by_status_class[k]` counts valid lines with status in `(k+1)*100 ..= (k+1)*100 + 99`.
R10. `total_bytes` is the sum of `bytes` over valid lines, saturating at `u64::MAX`.
R11. `latency_hist[i]` counts valid lines with `floor(log2(latency_us + 1)) == i`. Indexes above 31 are clamped to 31. `latency_us + 1` saturates at `u64::MAX`. Latencies 0, 1, 2, 3, 6, 7 land in buckets 0, 1, 1, 2, 2, 3.
R12. `slowest` is `None` without valid lines. Otherwise it holds the maximum `latency_us` and `line_offset`, the byte offset of the first byte of the first line with that latency. Offsets count from the first byte of the whole stream, across chunks.
R13. Distinct clients: `Aggregator::new()` allocates a table of `TABLE_SLOTS` (2^20) `u32` slots, open addressing, 0 means empty. The address `0.0.0.0` is never stored in the table. A separate flag records it.
R14. The table stores at most `MAX_STORED_CLIENTS` (943,718 = `2^20 * 9 / 10`) addresses. When a valid line carries an address that is not stored and the table is at that limit, `clients_overflow` increases by 1 and the address is not stored. Every such line counts, including repeats of the same rejected address. A stored address never counts as overflow.
R15. `distinct_clients_estimate` is the number of stored addresses plus 1 if `0.0.0.0` was seen. It is exact until the table fills.

**Streaming and memory**

R16. `ingest(chunk)` accepts chunks that end anywhere, including inside a line and including empty chunks. The result must not depend on how the stream is cut.
R17. A line that is incomplete at the end of a chunk is copied into a `[u8; CARRY_CAP]` array inside `Aggregator`. A line whose content passes `CARRY_CAP` bytes is counted malformed once, at that moment, and its remaining bytes are skipped up to the next `\n`.
R18. `finish()` flushes a final unterminated line, returns the `Summary`, and resets the aggregator to the state `new()` returns (table cleared, carry empty, offsets restart at 0).
R19. `Aggregator::new()` is the only place that allocates. `ingest` and `finish` perform 0 allocations and 0 reallocations. This includes the carry path, over-long lines and one-byte chunks.
R20. `baseline::aggregate(&str)` (given) produces the same `Summary` as the aggregator for any ASCII input.

## Given tests
17 tests run by default and 1 is `#[ignore]`.

| File | Test | Requirement |
|---|---|---|
| `tests/small.rs` | `empty_input` | R8, R12 |
| `tests/small.rs` | `hand_written_valid_lines` | R8 to R13, R15, R18 (exact `Summary`, last line unterminated, whole and 1-byte chunks) |
| `tests/small.rs` | `each_malformed_variant_is_counted_and_skipped` | R1 to R6 (22 variants: missing field, status `2OO`/`99`/`099`/`600`/`1000`, octet 256, bad octet counts, leading zero, negative and `+` numbers, extra field, double and trailing space, lowercase method, path without `/`, `\r`, `u64` overflow, empty line, lone space) |
| `tests/small.rs` | `accepted_edge_values` | R2, R3, R5, R10 |
| `tests/small.rs` | `slowest_tie_keeps_lowest_offset` | R12 |
| `tests/small.rs` | `latency_buckets_and_saturation` | R10, R11 |
| `tests/small.rs` | `line_length_limit_is_carry_cap` | R7, R17 |
| `tests/small.rs` | `offsets_count_from_stream_start_across_chunks` | R12, R16 (every chunk size from 1 to the input length) |
| `tests/small.rs` | `finish_resets_state` | R18 |
| `tests/small.rs` | `client_table_limit_and_overflow` | R13 to R15 (1,000,000 distinct addresses) |
| `tests/chunking.rs` | `generated_buffer_matches_baseline_for_every_chunk_size` | R16, R20 (100,000 generated lines, chunk sizes 1, 7, 4096, 65536, whole) |
| `tests/chunking.rs` | `random_chunk_boundaries_match_baseline` | R16, R20 (random chunk lengths) |
| `tests/chunking.rs` | `zero_length_chunks_are_ignored` | R16 |
| `tests/chunking.rs` | `mutated_input_matches_baseline` | R1 to R7, R20 (300 randomly corrupted buffers, fixed seed) |
| `tests/zero_alloc.rs` | `ingest_and_finish_allocate_nothing` | R19 (1,000,000 lines, 65,521-byte chunks) |
| `tests/zero_alloc.rs` | `tiny_chunks_and_long_lines_allocate_nothing` | R19 (1-byte chunks, 10,000-byte line) |
| `tests/zero_alloc.rs` | `counting_allocator_works` | Guards the counter against reading 0 for the wrong reason. It passes on the unmodified starter. |
| `tests/perf.rs` | `aggregator_beats_baseline_throughput` (`#[ignore]`) | Throughput, see Acceptance |

## Acceptance
1. `cargo test` exits 0 with `17 passed` in total across test binaries. On the unmodified starter, 16 of the 17 fail and `counting_allocator_works` passes.
2. `cargo clippy --all-targets -- -D warnings` exits 0.
3. Throughput: `cargo test --release -- --ignored --nocapture`. The test generates 5,000,000 lines (about 287 MB), takes the best of 2 baseline runs and the best of 3 aggregator runs, prints MB/s for both and the ratio, and asserts `ratio >= 1.5`.

The threshold is 1.5 and not higher because wall-clock ratios move on a loaded host. The reference solution measured 4.4x to 4.5x on the verifying machine (baseline about 124 MB/s, aggregator about 550 MB/s), about 3x the threshold. The same solution with a plain `iter().position` newline search measured about 3.5x. Record your own ratio in your notes. Do not run this test in debug mode; the ratio is meaningless there.

## Profiling instructions
Optional. The tests do not need them. These commands were not run while authoring this exercise (no `perf` on the authoring machine).

```
cargo install flamegraph
cargo flamegraph --release --test perf -- --ignored --nocapture
```

Or with `perf` directly (Linux). Take the test binary path from `cargo test --release --no-run`:

```
perf record -g target/release/deps/perf-<hash> --ignored
perf report --no-children
```

If `perf` is blocked, lower `kernel.perf_event_paranoid` or run as root. Look first for `malloc`/`free` frames (they should be absent from the aggregator), then for time in the newline search, number parsing and the table lookup.

## Extra tests to write
1. Chunk boundary on `\n`: split a two-line buffer exactly before and exactly after the first `\n`. Catches a carry buffer that is not cleared or a line parsed twice.
2. A 4,096-byte line delivered as 4,095 bytes in one chunk and the last byte plus `\n` in the next. Catches an off-by-one in the carry check on the completion path.
3. `\n\n\n` as input expects `malformed == 3`. Catches skipping empty lines.
4. Two over-long lines back to back, each split across several chunks, then a valid line. Expects `malformed == 2`, `lines == 1`. Catches a `discarding` flag that is not cleared at `\n`.
5. `0.0.0.0` alone: `distinct_clients_estimate == 1` and the table untouched. Catches a table that treats 0 as a stored key and loops or miscounts.
6. Addresses that collide in the table. Build 1,000 addresses whose hash has equal top 20 bits (search by brute force over your hash function) and check all 1,000 are counted. Catches broken probing.
7. A proptest or hand-rolled property: for random valid-line sets, `lines` equals the number of lines generated and `total_bytes` equals their sum. Catches a parser that is correct only on the generator's distributions.
8. Same stream ingested twice with `finish()` between, equal summaries (the given `finish_resets_state` checks this on a tiny input; repeat it on 100,000 generated lines).

## Stretch (optional)
1. Replace the newline search with `memchr::memchr` and report the ratio change. Then remove the second pass over each line: parse fields while scanning instead of splitting first.
2. Add `Summary::merge(&mut self, other: &Summary)` and aggregate in parallel: split the buffer at newline boundaries into one part per core, run one `Aggregator` per thread, merge. Decide what happens to `slowest.line_offset` (parts need base offsets), `distinct_clients_estimate` (the sets must be unioned, so each aggregator must expose its table) and `clients_overflow`.
3. Read the input with `mmap` (the `memmap2` crate or `libc::mmap`) instead of `read` into a `Vec`, and measure end to end on a file larger than RAM cache.
