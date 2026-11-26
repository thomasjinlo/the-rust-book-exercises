# Exercise 07: Async line server with limits and graceful shutdown

**Time:** ~4 h | **Area:** networking / async / concurrency | **Crate:** `crates/ex07-async-line-server`
**Builds on:** none (standalone). Exercises 03 and 04 are related (optional extension, never required).

## Read first
- Rust Book ch16, concurrency: https://doc.rust-lang.org/book/ch16-00-concurrency.html
- Rust Book ch17, async and await: https://doc.rust-lang.org/book/ch17-00-async-await.html (17.1 to 17.6; 17.1 is https://doc.rust-lang.org/book/ch17-01-futures-and-syntax.html and 17.6 is https://doc.rust-lang.org/book/ch17-06-futures-tasks-threads.html)
- Tokio tutorial, Spawning: https://tokio.rs/tokio/tutorial/spawning
- Tokio tutorial, Shared state: https://tokio.rs/tokio/tutorial/shared-state
- Tokio tutorial, I/O: https://tokio.rs/tokio/tutorial/io
- Tokio, Graceful shutdown: https://tokio.rs/tokio/topics/shutdown
- `TcpListener`: https://docs.rs/tokio/latest/tokio/net/struct.TcpListener.html
- `Semaphore`: https://docs.rs/tokio/latest/tokio/sync/struct.Semaphore.html
- `timeout`: https://docs.rs/tokio/latest/tokio/time/fn.timeout.html
- `AsyncBufReadExt`: https://docs.rs/tokio/latest/tokio/io/trait.AsyncBufReadExt.html

## What you are building
Every network daemon (a cache, a message broker, a metrics collector) has the same skeleton: accept connections, parse a framed protocol, and protect itself from clients that are too many, too slow or too large. You build that skeleton on tokio for a small text protocol. The server caps concurrent connections, closes idle ones, refuses lines that would need unbounded memory, and on shutdown stops accepting and drains in-flight connections within a grace period. `serve` returns counters that describe what happened.

## Starting point
- Edit: `crates/ex07-async-line-server/src/lib.rs`. The public API (`Config`, `Stats`, `serve`) is fixed. You may add modules and private items.
- Do not edit: `Cargo.toml`, `tests/`.
- Run from the crate directory: `cargo test`. Dependencies are already listed (`tokio` only).

## Requirements
Protocol. A request is one line. The server replies in request order.

1. `PING` is answered with `PONG\n`.
2. A line that starts with the 5 bytes `ECHO ` is answered with everything after those 5 bytes, followed by `\n`. The text may be empty. Bytes are echoed as received; the text need not be UTF-8.
3. Any other line is answered with `ERR unknown command\n` and the connection stays open. This includes `ECHO` (no space), `ECHOX hi`, lowercase `ping`, `PING ` (trailing space) and the empty line. Matching is case-sensitive and exact.
4. `QUIT` is answered with `BYE\n`, then the server closes the connection.
5. A line ends at `\n`. If the byte before `\n` is `\r`, exactly that one `\r` is removed. Any other `\r` is data.
6. Requests may arrive split at any byte boundary and many may arrive in one read. Each complete line is processed exactly once and in order. 1,000 requests sent in a single write produce 1,000 replies in order.

Limits.

7. A line is too long when its content, excluding the `\n` and the one stripped `\r`, is more than `max_line` bytes. A line of exactly `max_line` bytes is accepted, with or without `\r`.
8. The per-connection line buffer never holds more than `max_line + 2` bytes (content, `\r`, `\n`). The server reads from the socket in chunks of at most 4096 bytes. A client that sends bytes without a newline therefore gets its answer after at most `max_line + 2 + 4096` bytes have been consumed, however long the line would have been.
9. On a too-long line the server sends `ERR line too long\n`, closes the connection and counts it in `closed_too_long`. A too-long line is not counted in `commands`.
10. If no bytes are received on a connection for `idle_timeout`, the server sends `ERR idle timeout\n`, closes the connection and counts it in `closed_idle`. Any received byte restarts the timer. If the peer closes in the middle of a line, the partial line is discarded without a reply.
11. At most `max_connections` connections are served at the same time. A client that connects when that many are active receives `ERR busy\n` and is closed, and `rejected_busy` is incremented. Rejected connections do not count in `accepted`.
12. The server sends `ERR busy\n` without reading anything from the client. A rejected client that never writes still gets the message.
13. A slow or silent rejected client must not delay accepting other clients.
14. A connection's slot is released before its socket is closed. A client that sees the server close the connection and reconnects at once is never rejected because of that connection.

Shutdown and statistics.

15. When the `shutdown` future resolves, the server stops accepting at once and closes the listener. Existing connections keep working for up to `shutdown_grace`, including answering new commands. Connections still open when the grace ends are dropped (closed without a message). `serve` returns when every connection task has ended: as soon as the last connection ends, or after the grace, whichever comes first.
16. `serve` returns `Stats`: `accepted` (admitted connections), `rejected_busy`, `closed_idle`, `closed_too_long`, and `commands` (every complete line within `max_line` that was processed, including unknown commands and `QUIT`). Counts are exact for a scripted scenario.

## Given tests
17 tests in `tests/protocol.rs` (10) and `tests/limits.rs` (7). Each runs under a 60 s watchdog and binds `127.0.0.1:0`. They use real time with generous margins.

| Test | Requirement |
|---|---|
| `ping_returns_pong` | R1 |
| `echo_returns_text_after_first_space` | R2 |
| `unknown_commands_keep_the_connection_open` | R3 |
| `quit_says_bye_and_closes` | R4 |
| `crlf_is_stripped_once` | R5 |
| `one_byte_at_a_time` | R5, R6 |
| `thousand_pipelined_requests_answer_in_order` | R6, R16 |
| `line_at_the_limit_is_accepted_and_one_over_is_not` | R7, R9 |
| `oversized_line_is_cut_off_without_buffering_it` | R8, R9 |
| `random_sessions_with_random_splits_match_a_model` | R1 to R6 (40 seeded sessions, random splits of 1 to 7 bytes) |
| `third_client_is_busy_until_a_slot_frees` | R11, R12 |
| `idle_connection_gets_an_error_and_is_closed` | R10 |
| `shutdown_drops_an_idle_connection_when_the_grace_ends` | R15 |
| `connections_finish_their_work_during_the_grace_period` | R15 |
| `shutdown_stops_accepting_at_once` | R15 |
| `stats_for_a_scripted_scenario_are_exact` | R10, R11, R14, R16 |
| `two_hundred_clients_fifty_echoes_each` | R6, R11, R16 (200 clients, 10,200 commands) |

`oversized_line_is_cut_off_without_buffering_it` sends 4 KiB chunks of `x` with `max_line = 10_000`. After each chunk it waits up to 250 ms for the reply. It passes when the reply arrives after at most 6 chunks. A server that reads the whole line before checking the length never replies, and the client stops after 40 chunks (about 160 KiB of the 10 MiB it could send).

## Acceptance
From `crates/ex07-async-line-server`:

```
cargo test --no-fail-fast
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Pass condition: all three exit with code 0 and `cargo test` reports 17 passed in total across the two test files. On the unmodified starter, `cargo test --no-fail-fast` finishes in under 1 s with 17 failures. A full passing run takes about 2 s per test file.

Run the tests 20 times in a row before you call it done. Timing tests that pass once may not pass reliably.

## Extra tests to write
1. Idle timer restarts: idle 300 ms, send `PING` every 100 ms for 1 s, expect no `ERR idle timeout`. Catches a timer that measures total connection age.
2. Idle during a partial line: send `PIN`, wait past `idle_timeout`, expect `ERR idle timeout`. Catches a timeout that only runs between lines.
3. Peer closes mid-line: send `PIN`, close. Expect `accepted == 1`, `commands == 0` after shutdown and no panic. Catches counting or answering a partial line.
4. Pipelined limit breach: send `PING\n` followed by a too-long line in one write. Expect `PONG`, then `ERR line too long`. Catches checking the length of the whole read instead of each line.
5. `max_line = 0`: `PING` is too long, the empty line is an unknown command. Catches an underflow or an off-by-one at the boundary.
6. `max_connections = 0`: every client gets `ERR busy`. Catches a server that always admits one.
7. Busy clients that never read: open 50 connections beyond the limit without reading, then connect a valid client after freeing a slot. Catches the accept loop blocking on a rejected client (R13).
8. Non-UTF-8 echo: send `ECHO ` followed by bytes `0xFF 0xFE`. Expect the same bytes back. Catches use of `String`.

## Stretch (optional)
1. Per-IP limit: add `max_per_ip` to `Config`. Choose the reply text and write the tests for it first.
2. PROXY-style first line: optionally accept `PROXY <ip> <port>\n` as the first line and record the address it names. Decide how it interacts with `max_line` and the idle timeout.
3. Backpressure: connect a client that pipelines requests and never reads replies. Show with a test that the server's memory use stays bounded and that the connection is dropped at shutdown. Decide whether a write timeout belongs in `Config`.
