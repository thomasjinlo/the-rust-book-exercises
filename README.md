# Rust systems exercises

13 exercises on networking, concurrency, performance, storage and distributed systems. Each has a starter crate with given tests. Start after the Rust Book chapters listed in each exercise's "Read first".

## How to work

1. Read `exercises/NN-name.md`.
2. `cd crates/exNN-name && cargo test`. The starter fails until you implement it.
3. Edit only `src/` (files marked "do not edit" in the exercise stay as they are).
4. Done when the exercise's Acceptance section passes.

`solutions/` holds a reference solution for each exercise. Open it after you finish.

Every exercise stands alone. "Builds on" lines name an earlier exercise you can reuse. None is required.

## Order and time

| # | Exercise | Area | Time |
|---|---|---|---|
| 01 | [Rate limiter](exercises/01-rate-limiter.md) | systems / networking | 2 h |
| 02 | [LRU cache](exercises/02-lru-cache.md) | data structures | 3 h |
| 03 | [Frame codec](exercises/03-frame-codec.md) | wire protocols | 2.5 h |
| 04 | [Thread pool](exercises/04-thread-pool.md) | concurrency | 3 h |
| 05 | [SPSC ring buffer](exercises/05-spsc-ring.md) | lock-free, unsafe | 5 h |
| 06 | [Sharded concurrent map](exercises/06-sharded-map.md) | concurrency, performance | 3 h |
| 07 | [Async line server](exercises/07-async-line-server.md) | async networking | 4 h |
| 08 | [TCP reverse proxy](exercises/08-tcp-reverse-proxy.md) | networking, distributed | 6 h |
| 09 | [HTTP/1.1 request parser](exercises/09-http1-parser.md) | protocol parsing, security | 4 h |
| 10 | [Consistent hash ring](exercises/10-consistent-hash-ring.md) | distributed systems | 2.5 h |
| 11 | [CRDTs](exercises/11-crdts.md) | distributed systems | 3.5 h |
| 12 | [Write-ahead log](exercises/12-write-ahead-log.md) | storage, durability | 5 h |
| 13 | [Zero-allocation log aggregation](exercises/13-fast-log-aggregation.md) | performance | 4 h |

Suggested path after the Rust Book: 01, 02, 03, 04, 06, 07, then any of the rest. 05 and 12 are the longest unsafe and I/O exercises. 08 is the hardest.

## Maintainers

`scripts/verify-solutions.sh [exNN-name ...]` copies each starter to a scratch directory, overlays the reference solution's `src/`, and runs `cargo test`.
