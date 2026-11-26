# Rust systems exercises

9 exercises on networking, concurrency, performance, storage and distributed systems. Each has a starter crate with given tests, a concepts section with a worked example, and milestones you can check one at a time. Each takes 3 hours or less. Most prerequisite reading is the Rust Book.

## How to work

1. Read `exercises/NN-name.md`, including "Concepts" and "Milestones".
2. `cd crates/exNN-name && cargo test`. The starter fails until you implement it.
3. Edit only the files the exercise names. Never edit `tests/`.
4. Done when the exercise's Acceptance section passes.

`solutions/` holds a reference solution for each exercise. Open it after you finish.

Every exercise stands alone. A "Builds on" line names an earlier exercise you can reuse. None is required.

## Order and time

The order follows the Rust Book chapters you need.

| # | Exercise | Area | Time | Rust Book chapters |
|---|---|---|---|---|
| 01 | [Rate limiter](exercises/01-rate-limiter.md) | systems / networking | 2 h | 4, 5, 6, 8, 9, 10 |
| 02 | [LRU cache](exercises/02-lru-cache.md) | data structures | 2.5 h | 4, 8, 10, 11, 15 |
| 03 | [Consistent hash ring](exercises/03-consistent-hash-ring.md) | distributed systems | 2.5 h | 5, 8, 10, 11, 13 |
| 04 | [Frame codec](exercises/04-frame-codec.md) | wire protocols | 2 h | 4, 6, 8, 9, 11 |
| 05 | [Write-ahead log](exercises/05-write-ahead-log.md) | storage, durability | 3 h | 4, 5, 6, 8, 9, 10, 11, 12 |
| 06 | [Thread pool](exercises/06-thread-pool.md) | concurrency | 2.5 h | 15, 16, 21 |
| 07 | [Sharded concurrent map](exercises/07-sharded-map.md) | concurrency, performance | 2.5 h | 8, 10, 15, 16 |
| 08 | [Async line server](exercises/08-async-line-server.md) | async networking | 3 h | 15, 16, 17 |
| 09 | [Zero-allocation log aggregation](exercises/09-fast-log-aggregation.md) | performance | 3 h | 4, 8, 10, 11, 13 |

Times are estimates, not measured learner times.

## Maintainers

`scripts/verify-solutions.sh [exNN-name ...]` copies each starter to a scratch directory, overlays the reference solution's `src/`, and runs `cargo test`.

`archive/` holds the first, larger set of 13 exercises (adds a SPSC ring buffer, a TCP reverse proxy, an HTTP/1.1 parser and CRDTs, and has longer versions of the others). Nothing in it is maintained.
