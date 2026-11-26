# Rust systems exercises

9 exercises on networking, concurrency, performance, storage and distributed systems. Each is a 2-hour lab: a worked example, the 2-4 calls to build, one hard rule, and black-box tests. The design is yours. Most prerequisite reading is the Rust Book.

## How to work

1. Read `exercises/NN-name.md`.
2. `cd crates/exNN-name && cargo test`. The starter fails until you implement it.
3. Edit only the files the exercise names. Never edit `tests/`.
4. Done when the exercise's "Done when" section passes.

`solutions/` holds a reference solution for each exercise. Open it after you finish.

Every exercise stands alone. A "Builds on" line names an earlier exercise you can reuse. None is required.

## Order and time

The order follows the Rust Book chapters you need.

| # | Exercise | Area | Time | Rust Book sections |
|---|---|---|---|---|
| 01 | [Rate limiter](exercises/01-rate-limiter.md) | systems / networking | 2 h | 3.2, 5.3 |
| 02 | [LRU cache](exercises/02-lru-cache.md) | data structures | 2 h | 8.3, 10.1 |
| 03 | [Consistent hash ring](exercises/03-consistent-hash-ring.md) | distributed systems | 2 h | 8.3, 10.3 |
| 04 | [Frame codec](exercises/04-frame-codec.md) | wire protocols | 2 h | 3.2, 4.3 |
| 05 | [Write-ahead log](exercises/05-write-ahead-log.md) | storage, durability | 2 h | 9.2, 12.2 |
| 06 | [Thread pool](exercises/06-thread-pool.md) | concurrency | 2 h | 16.2, 16.3 |
| 07 | [Sharded concurrent map](exercises/07-sharded-map.md) | concurrency, performance | 2 h | 8.3, 16.3 |
| 08 | [Async line server](exercises/08-async-line-server.md) | async networking | 2 h | 17.1, 17.2 |
| 09 | [Zero-allocation log aggregation](exercises/09-fast-log-aggregation.md) | performance | 2 h | 4.3, 13.2 |

Times are estimates, not measured learner times.

## Maintainers

`scripts/verify-solutions.sh [exNN-name ...]` copies each starter to a scratch directory, overlays the reference solution's `src/`, and runs `cargo test`.

`archive/` holds the first, larger set of 13 exercises (adds a SPSC ring buffer, a TCP reverse proxy, an HTTP/1.1 parser and CRDTs, and has longer versions of the others). Nothing in it is maintained.
