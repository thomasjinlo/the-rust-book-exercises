# Exercise 08: Async line server

**Time:** 3 h | **Crate:** `crates/ex08-async-line-server`

## Read first
**Rust Book**
- [Ch. 9.2: Recoverable Errors with Result](https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html)
- [Ch. 10.2: Defining Shared Behavior with Traits](https://doc.rust-lang.org/book/ch10-02-traits.html)
- [Ch. 16.2: Transfer Data Between Threads with Message Passing](https://doc.rust-lang.org/book/ch16-02-message-passing.html)
- [Ch. 16.3: Shared-State Concurrency](https://doc.rust-lang.org/book/ch16-03-shared-state.html)
- [Ch. 17: Fundamentals of Asynchronous Programming: Async, Await, Futures, and Streams](https://doc.rust-lang.org/book/ch17-00-async-await.html)
- [Ch. 17.1: Futures and the Async Syntax](https://doc.rust-lang.org/book/ch17-01-futures-and-syntax.html)
- [Ch. 17.2: Applying Concurrency with Async](https://doc.rust-lang.org/book/ch17-02-concurrency-with-async.html)
- [Ch. 17.3: Working With Any Number of Futures](https://doc.rust-lang.org/book/ch17-03-more-futures.html)
- [Ch. 17.5: A Closer Look at the Traits for Async](https://doc.rust-lang.org/book/ch17-05-traits-for-async.html)
- [Ch. 17.6: Futures, Tasks, and Threads](https://doc.rust-lang.org/book/ch17-06-futures-tasks-threads.html)

**Standard library**
- [`Future`](https://doc.rust-lang.org/std/future/trait.Future.html)

**Background**
- [Spawning (Tokio tutorial)](https://tokio.rs/tokio/tutorial/spawning): an accept loop that starts one task per connection, so one slow client does not block the others.
- [Framing (Tokio tutorial)](https://tokio.rs/tokio/tutorial/framing): why one socket read can hold half a request or several, and how buffering turns a byte stream into requests.
- [mini-redis/src/server.rs (GitHub)](https://github.com/tokio-rs/mini-redis/blob/master/src/server.rs): a small production-style server with a cap on concurrent connections, a per-connection task and a shutdown that waits for open connections.

The Book teaches the language feature (futures, `.await`). The server itself runs on the `tokio` runtime, which is already a dependency of the crate; the given tests use `#[tokio::test]` to start it.

## Task
Build a TCP server for a small text protocol. It answers one request per line, refuses lines that are too long and
serves only a fixed number of clients at once. When told to shut down, it lets open connections finish and then
reports counters.

## API
Every reply ends with `\n`. No call may panic, for any input. Run on the `tokio` runtime.

### `Config`
```rust
pub struct Config {
    pub max_connections: usize,
    pub max_line: usize,
}
```

### `Stats`
```rust
pub struct Stats {
    pub accepted: u64,
    pub rejected_busy: u64,
    pub commands: u64,
}
```
1. `accepted` is the number of connections that were served.
2. `rejected_busy` is the number of connections that were answered `ERR busy`.
3. `commands` is the number of lines that got a reply from the table below, including `ERR unknown command` and
   `QUIT`. A too-long line, a partial line and a `busy` rejection are not counted.

### `serve`
```rust
pub async fn serve(
    listener: tokio::net::TcpListener,
    config: Config,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> Stats
```
Serves `listener` until `shutdown` resolves, then returns the final `Stats`. A request is one line. Replies are
sent in request order.

| Client sends | Server replies | Connection |
|---|---|---|
| `PING` | `PONG` | stays open |
| `ECHO ` followed by any bytes (may be none) | those bytes, as received (need not be UTF-8) | stays open |
| `QUIT` | `BYE` | server closes |
| any other line, including `ECHO`, `ECHOX hi`, `ping`, `PING ` and the empty line | `ERR unknown command` | stays open |
| a line longer than `max_line` | `ERR line too long` | server closes |
| (any or no bytes) while `max_connections` connections are being served | `ERR busy` | server closes |

Matching is case-sensitive and exact.

Lines:
1. A line ends at `\n`. If the byte before `\n` is `\r`, that one `\r` is removed. Any other `\r` is data:
   `ECHO abc\r\r\n` is answered `abc\r`.
2. Requests may arrive split at any byte boundary, and many may arrive in one write. Each complete line is
   processed once, in order. 1,000 requests sent in one write get 1,000 replies in order.
3. If the client closes in the middle of a line, that partial line gets no reply.
4. A line is too long when its content (without `\n` and the removed `\r`) is more than `max_line` bytes. Exactly
   `max_line` bytes is accepted, with or without `\r`.
5. A client that sends 1 MiB with no `\n` when `max_line` is 1024 gets `ERR line too long`, then the close.

Connections:
6. A client that connects while `max_connections` connections are being served gets `ERR busy` without sending
   anything. Such a connection does not count as accepted.
7. A slot is free as soon as a client sees the server close its connection (after `QUIT`, a too-long line or
   `ERR busy`) or the client closes. With `max_connections = 1`, 50 reconnects in a row after a server close are
   all served.

Shutdown:
8. After `shutdown` resolves, new clients are refused or get no reply. Open connections keep working and get
   replies to new lines.
9. `serve` returns only after `shutdown` has resolved and every open connection has ended. It never closes a
   connection by itself, apart from the cases in the table.

## Example
`max_connections = 2`, `max_line = 16`:

| Client | Sends | Server replies | Then |
|---|---|---|---|
| A | `PING\nECHO hi\nFOO\nQUIT\n` | `PONG`, `hi`, `ERR unknown command`, `BYE` | server closes |
| B | `xxxxxxxxxxxxxxxxx\n` (17 `x`) | `ERR line too long` | server closes |
| C | `PING\n` | `PONG` | stays open |
| D | `PING\n` | `PONG` | stays open |
| E (C and D open) | nothing | `ERR busy` | server closes |

Then `shutdown` resolves and C and D close. `serve` returns
`Stats { accepted: 4, rejected_busy: 1, commands: 6 }`.

## Done when
Edit only `src/lib.rs` (you may add modules). Do not edit `Cargo.toml` or `tests/`. The starter compiles, and all
15 of its tests fail until you finish.

From `crates/ex08-async-line-server`, all of these exit 0:
```
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```
`cargo test` reports 15 passed.

## Extra tests (optional)
- Client sends `PIN` and closes. After shutdown: `accepted == 1`, `commands == 0`.
- One write of `PING\n` and then a too-long line. Replies: `PONG`, `ERR line too long`.
- `max_line = 0`: `PING\n` gets `ERR line too long`, and `\n` gets `ERR unknown command`.
- `max_connections = 0`: every client gets `ERR busy`.
- `ECHO ` followed by bytes `0xFF 0xFE` is answered with `0xFF 0xFE`.

## Stretch (optional)
- Close a connection that sends nothing for a configurable time.
- After shutdown, close open connections after N seconds.
- A client pipelines requests and never reads: keep server memory bounded.
