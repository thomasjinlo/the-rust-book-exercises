# Exercise 08: Async line server

**Time:** 2 h | **Crate:** `crates/ex08-async-line-server`

## Task
Build a TCP server for a small text protocol: one request per line, one reply per request. It serves only a fixed
number of clients at once.

## Example
One client sends `PING\nECHO hi\nFOO\nQUIT\n` in a single write.

| Client sends | Server replies | Connection |
|---|---|---|
| `PING\n` | `PONG\n` | stays open |
| `ECHO hi\n` | `hi\n` | stays open |
| `FOO\n` | `ERR unknown command\n` | stays open |
| `QUIT\n` | `BYE\n` | server closes |

## API
Runs on the `tokio` runtime.

### `serve`
```rust
pub async fn serve(listener: tokio::net::TcpListener, max_connections: usize)
```
Serves clients until the task running it is dropped or aborted.
1. A request is one line, ending at `\n`. A request may arrive in pieces, and many may arrive in one write. Each
   complete line is answered once, in order. A partial line at close gets no reply.
2. `PING` is answered `PONG`. `ECHO <text>` is answered `<text>`. `QUIT` is answered `BYE` and the server closes.
   Any other line, including `ECHO`, `ping` and the empty line, is answered `ERR unknown command`.
3. Every reply ends with `\n`. Matching is case-sensitive and exact.
4. A client that connects while `max_connections` clients are being served gets `ERR busy\n` and is closed,
   without sending anything. A slot is free again when a served client closes or is closed by the server.

## Done when
Edit only `src/lib.rs`. From `crates/ex08-async-line-server`, `cargo test` reports 9 passed (on the starter, 9 fail).
The hard rule: with `max_connections = 2`, a third client gets `ERR busy` and a close, and after one of the first
two leaves, a new client is served.

## Read first
- [Ch. 17.1: Futures and the Async Syntax](https://doc.rust-lang.org/book/ch17-01-futures-and-syntax.html)
- [Ch. 17.2: Applying Concurrency with Async](https://doc.rust-lang.org/book/ch17-02-concurrency-with-async.html)
- [mini-redis/src/server.rs (GitHub)](https://github.com/tokio-rs/mini-redis/blob/master/src/server.rs): a small server with a cap on concurrent connections and one task per connection.

## Stretch (optional)
- Refuse lines longer than a configured maximum with `ERR line too long`, then close.
- Add a shutdown future to `serve` that lets open connections finish before it returns.
