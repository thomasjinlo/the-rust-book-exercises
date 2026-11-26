# Exercise 08: Async line server with a connection limit

**Time:** ~3 h | **Area:** networking / async / concurrency | **Crate:** `crates/ex08-async-line-server`
**Builds on:** none (standalone). Exercises 04 and 06 are related (optional extension, never required).

## Read first
- Rust Book ch15, smart pointers: https://doc.rust-lang.org/book/ch15-00-smart-pointers.html
- Rust Book ch16, concurrency: https://doc.rust-lang.org/book/ch16-00-concurrency.html
  - 16.1 threads: https://doc.rust-lang.org/book/ch16-01-threads.html
  - 16.2 message passing: https://doc.rust-lang.org/book/ch16-02-message-passing.html
  - 16.3 shared state: https://doc.rust-lang.org/book/ch16-03-shared-state.html
- Rust Book ch17, async and await: https://doc.rust-lang.org/book/ch17-00-async-await.html
  - 17.1 futures and syntax: https://doc.rust-lang.org/book/ch17-01-futures-and-syntax.html
  - 17.2 concurrency with async: https://doc.rust-lang.org/book/ch17-02-concurrency-with-async.html
  - 17.3 more futures: https://doc.rust-lang.org/book/ch17-03-more-futures.html
  - 17.4 streams: https://doc.rust-lang.org/book/ch17-04-streams.html
  - 17.5 traits for async: https://doc.rust-lang.org/book/ch17-05-traits-for-async.html
  - 17.6 futures, tasks and threads: https://doc.rust-lang.org/book/ch17-06-futures-tasks-threads.html
- Tokio tutorial, Spawning: https://tokio.rs/tokio/tutorial/spawning
- Tokio tutorial, I/O: https://tokio.rs/tokio/tutorial/io
- `TcpListener`: https://docs.rs/tokio/latest/tokio/net/struct.TcpListener.html
- `Semaphore`: https://docs.rs/tokio/latest/tokio/sync/struct.Semaphore.html
- `AsyncBufReadExt`: https://docs.rs/tokio/latest/tokio/io/trait.AsyncBufReadExt.html

## What you are building
Every network daemon (a cache, a message broker, a metrics collector) has the same skeleton: accept connections, split a byte stream into request lines, and protect itself from clients that are too many or too large. You build that skeleton on tokio for a small text protocol. The server caps concurrent connections, refuses lines that would need unbounded memory, and on shutdown stops accepting and waits for open connections to finish. `serve` returns counters that describe what happened.

## Concepts
**Where tokio fits.** The Book teaches `async`/`await` with its own helper crate. Use the Book chapter to learn the language feature: futures, `.await`, and why a future does nothing until something polls it. The Tokio tutorial covers the runtime that polls them. Tokio supplies three things here:
- `#[tokio::main]` and `#[tokio::test]` wrap your `async fn` in a runtime and block until it finishes. The given tests use `#[tokio::test]`.
- `tokio::spawn(fut)` starts `fut` as an independent task on the runtime and returns a `JoinHandle`. The task keeps running if you never await the handle.
- Async versions of sockets, timers and channels (`TcpListener`, `Semaphore`, `AsyncBufReadExt`). They suspend the task instead of blocking a thread.

A complete echo server (not this exercise, but the same pattern):
```rust
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    loop {
        let (mut socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            while let Ok(n) = socket.read(&mut buf).await {
                if n == 0 || socket.write_all(&buf[..n]).await.is_err() {
                    break;
                }
            }
        });
    }
}
```
The accept loop never waits for a client: each connection is a task.

**A byte stream has no message boundaries.** TCP delivers bytes, not lines. One read may return half a line, or ten lines. You must buffer and cut at `\n`:
```
client writes:   "PING\nECHO h"   then   "i\r\nQUIT\n"
server reads:    "PING\nECHO h"   -> line "PING" (reply PONG), "ECHO h" stays buffered
server reads:    "i\r\nQUIT\n"    -> line "ECHO hi\r" -> strip one \r -> reply "hi"
                                     line "QUIT"      -> reply BYE, close
```
**Why a limit on line length.** A client that sends gigabytes without `\n` makes a naive `read_line` grow its buffer until the process dies. The server must stop reading at `max_line + 2` bytes.

**Why a connection limit.** Each connection holds a socket and a task. A `Semaphore` with `max_connections` permits counts them: `try_acquire_owned` takes a permit without waiting, and dropping the permit gives it back.

**Shutdown.** `serve` takes a `shutdown` future. When it resolves, stop calling `accept`, drop the listener, then wait for every connection task to finish. Clients must close their connections for `serve` to return. There is no timer and no forced close.

## Milestones
Run the given tests with `cargo test <filter>` from the crate directory. Test files: `tests/protocol.rs`, `tests/limits.rs`.

1. **Accept loop and commands** (R1 to R4). Bind-and-accept loop, one `tokio::spawn` per connection, `PING`, `ECHO`, `QUIT`, unknown. Go green: `ping_returns_pong`, `echo_returns_text_after_first_space`, `unknown_commands_keep_the_connection_open`, `quit_says_bye_and_closes`. Run: `cargo test --test protocol -- ping echo unknown quit`.
2. **Framing** (R5, R6). Cut lines at `\n`, strip one `\r`, handle split and pipelined input. Go green: `crlf_is_stripped_once`, `one_byte_at_a_time`, `thousand_pipelined_requests_answer_in_order`. Run: `cargo test --test protocol -- crlf one_byte pipelined`.
3. **Line limit** (R7 to R9). Bounded read, `ERR line too long`, close. Go green: `line_at_the_limit_is_accepted_and_one_over_is_not`, `one_mebibyte_line_without_newline_is_refused`. Run: `cargo test --test protocol` (all 9 pass).
4. **Connection limit** (R10 to R12). Semaphore, `ERR busy`. Go green: `third_client_is_busy_until_a_slot_frees`. Run: `cargo test --test limits busy`.
5. **Shutdown and stats** (R13, R14). Go green: `shutdown_waits_for_open_connections`, `shutdown_stops_accepting`, `stats_for_a_scripted_scenario_are_exact`, `two_hundred_clients_fifty_echoes_each`. Run: `cargo test` (all 14 pass).

## Starting point
- Edit: `crates/ex08-async-line-server/src/lib.rs`. The public API (`Config`, `Stats`, `serve`) is fixed. You may add modules and private items.
- Do not edit: `Cargo.toml`, `tests/`.
- Run from the crate directory: `cargo test`. Dependencies are already listed (`tokio` only).

## Requirements
Protocol. A request is one line. The server replies in request order.

1. `PING` is answered with `PONG\n`.
2. A line that starts with the 5 bytes `ECHO ` is answered with everything after those 5 bytes, followed by `\n`. The text may be empty. Bytes are echoed as received; the text need not be UTF-8.
3. Any other line is answered with `ERR unknown command\n` and the connection stays open. This includes `ECHO` (no space), `ECHOX hi`, lowercase `ping`, `PING ` (trailing space) and the empty line. Matching is case-sensitive and exact.
4. `QUIT` is answered with `BYE\n`, then the server closes the connection.
5. A line ends at `\n`. If the byte before `\n` is `\r`, exactly that one `\r` is removed. Any other `\r` is data.
6. Requests may arrive split at any byte boundary and many may arrive in one read. Each complete line is processed exactly once and in order. 1,000 requests sent in a single write produce 1,000 replies in order. If the peer closes in the middle of a line, the partial line is discarded without a reply.

Line limit.

7. A line is too long when its content, excluding the `\n` and the one stripped `\r`, is more than `max_line` bytes. A line of exactly `max_line` bytes is accepted, with or without `\r`.
8. The per-connection line buffer never holds more than `max_line + 2` bytes (content, `\r`, `\n`), however many bytes the client sends without a newline.
9. On a too-long line the server sends `ERR line too long\n` and closes the connection. A too-long line is not counted in `commands`.

Connection limit.

10. At most `max_connections` connections are served at the same time. A client that connects when that many are active receives `ERR busy\n` and is closed, and `rejected_busy` is incremented. Rejected connections do not count in `accepted`.
11. The server sends `ERR busy\n` without reading anything from the client. A rejected client that never writes still gets the message.
12. A connection's slot is released before its socket is closed. A client that sees the server close the connection and reconnects at once is never rejected because of that connection.

Shutdown and statistics.

13. When the `shutdown` future resolves, the server stops accepting and closes the listener. Open connections keep working, including answering new commands. `serve` returns once every connection task has ended. Clients must close their connections; the server does not time out or abort them.
14. `serve` returns `Stats`: `accepted` (admitted connections), `rejected_busy`, and `commands` (every complete line within `max_line` that was processed, including unknown commands and `QUIT`). Counts are exact for a scripted scenario.

## Common mistakes
- Using `read_line` on a plain `BufReader`. It grows its buffer without bound on a line with no `\n`. Cap the read, for example with `AsyncReadExt::take` and `read_until`.
- Assuming one `read` returns one request. Tests send one byte at a time and 1,000 lines in one write.
- Counting the `\r` or the `\n` toward `max_line`, or stripping every trailing `\r` instead of one.
- Awaiting the client inside the accept loop. One slow client then blocks everyone. Spawn a task per connection, and for `ERR busy` as well.
- Releasing the semaphore permit after closing the socket. A client that reconnects at once is then rejected.
- Returning from `serve` right after the shutdown future resolves. Wait for the connection tasks (`JoinSet` is a convenient holder).

## Given tests
14 tests in `tests/protocol.rs` (9) and `tests/limits.rs` (5). Each runs under a 60 s watchdog and binds `127.0.0.1:0`. Tests that wait use 10 s limits, far above the milliseconds a loopback reply takes.

| Test | Requirement |
|---|---|
| `ping_returns_pong` | R1 |
| `echo_returns_text_after_first_space` | R2 |
| `unknown_commands_keep_the_connection_open` | R3 |
| `quit_says_bye_and_closes` | R4 |
| `crlf_is_stripped_once` | R5 |
| `one_byte_at_a_time` | R5, R6 |
| `thousand_pipelined_requests_answer_in_order` | R6, R14 |
| `line_at_the_limit_is_accepted_and_one_over_is_not` | R7, R9 |
| `one_mebibyte_line_without_newline_is_refused` | R8, R9 |
| `third_client_is_busy_until_a_slot_frees` | R10, R11 |
| `shutdown_waits_for_open_connections` | R13 |
| `shutdown_stops_accepting` | R13 |
| `stats_for_a_scripted_scenario_are_exact` | R9, R10, R12, R14 |
| `two_hundred_clients_fifty_echoes_each` | R6, R10, R14 (200 clients, 10,200 commands) |

## Acceptance
From `crates/ex08-async-line-server`:

```
cargo test --no-fail-fast
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Pass condition: all three exit with code 0 and `cargo test` reports 14 passed in total across the two test files. On the unmodified starter, `cargo test --no-fail-fast` finishes in under 1 s with 14 failures. A full passing run takes about 1.5 s.

Run the tests 20 times in a row before you call it done.

## Extra tests to write
1. Peer closes mid-line: send `PIN`, close. Expect `accepted == 1`, `commands == 0` after shutdown. Catches counting or answering a partial line.
2. Pipelined limit breach: send `PING\n` followed by a too-long line in one write. Expect `PONG`, then `ERR line too long`. Catches checking the length of the whole read instead of each line.
3. `max_line = 0`: `PING` is too long, the empty line is an unknown command. Catches an underflow or an off-by-one at the boundary.
4. `max_connections = 0`: every client gets `ERR busy`. Catches a server that always admits one.
5. Busy clients that never read: open 50 connections beyond the limit without reading, then connect a valid client after freeing a slot. Catches an accept loop that blocks on a rejected client.
6. Non-UTF-8 echo: send `ECHO ` followed by bytes `0xFF 0xFE`. Expect the same bytes back. Catches use of `String`.

## Stretch (optional)
1. Idle timeout: close a connection that sends nothing for a configurable time, with `tokio::time::timeout`. Decide whether a half-sent line counts as activity.
2. Shutdown grace: after the signal, give connections N seconds, then abort them (`JoinSet::abort_all`). Write the tests first.
3. Backpressure: connect a client that pipelines requests and never reads replies. Show with a test that server memory stays bounded.
