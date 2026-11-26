# Exercise 08: A layer-4 TCP reverse proxy with passive health checks

**Time:** ~6 h | **Area:** networking / async / distributed systems | **Crate:** `crates/ex08-tcp-reverse-proxy`
**Builds on:** exercise 07 for async server structure (optional extension, never required)

## Read first
- Rust Book, async chapter: https://doc.rust-lang.org/book/ch17-00-async-await.html
- Tokio tutorial: https://tokio.rs/tokio/tutorial
- `tokio::io::copy_bidirectional`: https://docs.rs/tokio/1/tokio/io/fn.copy_bidirectional.html
- `tokio::net::TcpStream` (`shutdown`, `set_nodelay`): https://docs.rs/tokio/1/tokio/net/struct.TcpStream.html
- `tokio::time::timeout`: https://docs.rs/tokio/1/tokio/time/fn.timeout.html
- `tokio::sync::watch`: https://docs.rs/tokio/1/tokio/sync/watch/index.html
- Load balancing background: https://en.wikipedia.org/wiki/Load_balancing_(computing)
- TCP half-close (FIN, `shutdown(Write)`): https://en.wikipedia.org/wiki/Transmission_Control_Protocol#Connection_termination

## What you are building
A layer-4 reverse proxy accepts TCP connections and pipes the bytes to one of several backend servers without looking at the protocol. Load balancers and service meshes use this to spread connections over a pool. Passive health checking watches real connection attempts instead of probing: a backend that keeps failing is ejected for a cooldown, then given a single trial connection. Your proxy does round-robin selection, failover, ejection with trial reinstatement, half-close-correct forwarding, exact byte accounting, and a graceful shutdown.

## Starting point
- Edit `src/lib.rs`. You may add modules under `src/`. Keep the public API unchanged: `Connector`, `TokioConnector`, `ProxyConfig`, `ProxyConfig::new`, `ProxyStats`, `Proxy::{start, start_with_connector, local_addr, stats, shutdown}`, `SHUTDOWN_GRACE`.
- `TokioConnector` and `ProxyConfig::new` are complete. Do not edit them.
- Do not edit `tests/` or `Cargo.toml`. The `[profile.dev] opt-level = 2` setting is deliberate: the 64 MiB test hashes every byte.
- The test helpers live in `tests/common/mod.rs`. Read them: `Fake` is a `Connector` that records attempts and can refuse, hang, delay or gate a connect per backend address.
- Run: `cargo test --no-fail-fast` from the crate directory. The unmodified starter fails every test within a second (the stubs call `todo!()`).

## Requirements
Terms: the *accept ordinal* of a connection is its 0-based position in the order `accept` returned it. *Attempt* means one call to `Connector::connect` for one backend on behalf of one client connection.

**Selection and failover**
- **R1.** Connection with accept ordinal `k` starts at backend index `k % backends.len()`, where index 0 is `config.backends[0]`. The ordinal advances by one per accepted connection, whatever happened to earlier connections (failover does not shift it).
- **R2.** If an attempt fails, try the next backends in index order, wrapping around. Each backend is tried at most once per client connection. At most `backends.len()` attempts are made.
- **R3.** An attempt fails if `Connector::connect` returns `Err`, or does not finish within `connect_timeout`. On timeout drop the connect future.
- **R4.** Every failed attempt adds 1 to `backend_connect_failures` and 1 to that backend's consecutive-failure counter.
- **R5.** Accepting never waits for a connect. Selection and connecting run in a per-connection task, but the start index of R1 is fixed in the accept loop.

**Passive health**
- **R6.** A backend whose consecutive-failure counter reaches `eject_after_failures` is *ejected* for `eject_cooldown`. A value of 0 behaves like 1.
- **R7.** An ejected backend is skipped: no attempt is made, no failure is counted, and it does not count toward the "at most once" limit of R2 beyond being passed over.
- **R8.** A successful attempt sets that backend's counter to 0.
- **R9.** When the cooldown has elapsed, exactly one connection may attempt the backend (the *trial*). While the trial attempt is outstanding, every other connection skips the backend.
- **R10.** A successful trial reinstates the backend (counter 0, no longer ejected). A failed trial ejects it again for a fresh `eject_cooldown`.
- **R11.** A task that is aborted while holding a trial must not leave the backend ejected forever. (Aborts happen in `shutdown`; the rule matters for any design that cancels connection tasks.)
- **R12.** Use `tokio::time::Instant` for all health timestamps.

**No backend**
- **R13.** If no attempt succeeds, including when every backend is ejected or the list is empty, close the client socket and add 1 to `no_backend_available`. Increment the counter before closing, so a client that sees the close also sees the counter.
- **R14.** Never read from the client socket before a backend connection exists. A client that wrote data before the proxy gave up must find that data unread, which the kernel reports as a reset (see the given test).

**Forwarding**
- **R15.** Copy bytes in both directions concurrently. When one side reaches EOF, call `shutdown` on the write half of the other socket and keep copying the opposite direction until it also ends. The client can half-close and still receive the response; the backend can too.
- **R16.** Set `TCP_NODELAY` on the client socket and the backend socket before copying.
- **R17.** `bytes_client_to_backend` and `bytes_backend_to_client` count payload bytes successfully written to the backend and to the client. For a connection that ends cleanly the counts are exact. Counts must be added before `completed` is incremented for that connection.
- **R18.** `completed` counts connections that reached a backend and whose forwarding task has finished, whether cleanly or on an I/O error. Connections that never reached a backend (R13) and connections aborted by `shutdown` are not counted.
- **R19.** A transient `accept` error does not stop the accept loop.

**Lifecycle and API**
- **R20.** `Proxy::start` must be called inside a Tokio runtime. It spawns the accept loop and returns at once. `start` uses `TokioConnector`. `start_with_connector` uses the connector you pass.
- **R21.** `local_addr` returns the listener's address. `stats` returns a snapshot of six counters, all 0 at start, none ever decreasing. If a snapshot shows a connection in `completed`, it also shows that connection's bytes.
- **R22.** `shutdown(self)` closes the listening socket immediately, then waits for in-flight connections for up to `SHUTDOWN_GRACE` (2 s). Connections still running after that are aborted, including ones still inside connect. `shutdown` returns after every task the proxy spawned has ended. Connections that finish within the grace are served normally, and `shutdown` returns as soon as the last one ends.
- **R23.** Finished connection tasks are reaped while the proxy runs. Memory must not grow with the number of past connections.

A note on time: the cooldown tests use real time (a 1 s cooldown). Tokio's paused clock auto-advances whenever the runtime is idle, including while a socket read is pending, so it can fire `connect_timeout` early. The given tests do not use it.

## Given tests
27 tests in four files.

| File / test | Requirement |
|---|---|
| balancing: `three_hundred_connections_hit_each_backend_exactly_100_times` | R1, R5, R18 |
| balancing: `first_connection_goes_to_backend_zero_then_cycles_in_order` | R1 |
| balancing: `failover_tries_following_backends_in_order_each_at_most_once` | R2, R4 |
| balancing: `all_backends_down_closes_the_client_and_counts_it` | R2, R13 |
| balancing: `zero_backends_closes_every_client` | R13 |
| balancing: `randomized_outage_patterns_match_the_round_robin_model` (seeded, 8 rounds) | R1, R2, R4, R13 |
| balancing: `a_slow_connect_does_not_block_other_connections` | R5 |
| balancing: `local_addr_is_the_listener_address_and_stats_start_at_zero` | R20, R21 |
| health: `connect_timeout_counts_as_a_failure_and_fails_over` | R3, R4 |
| health: `real_refused_port_is_skipped_then_ejected` | R4, R6, R7 (real sockets) |
| health: `ejected_backend_gets_no_attempts_during_the_cooldown` | R6, R7 |
| health: `a_success_resets_the_consecutive_failure_counter` | R8 |
| health: `after_the_cooldown_one_trial_decides_and_failure_re_ejects` | R9, R10 |
| health: `a_successful_trial_reinstates_the_backend` | R10 |
| health: `only_one_trial_is_in_flight_at_a_time` | R9 |
| forwarding: `client_half_close_reaches_the_backend_which_can_still_reply` | R15, R17 |
| forwarding: `backend_half_close_reaches_the_client_which_can_still_send` | R15 |
| forwarding: `greeting_then_close_backend_is_delivered_in_full` | R15, R17 |
| forwarding: `byte_counters_are_exact` | R17, R18 |
| forwarding: `randomized_payloads_and_write_splits_round_trip_exactly` (seeded, 60 connections) | R15, R17 |
| forwarding: `tcp_nodelay_is_set_on_the_backend_socket` | R16 (backend side only) |
| forwarding: `client_bytes_are_not_consumed_before_a_backend_is_connected` | R14 |
| forwarding: `sixty_four_mib_both_directions_at_once_keep_their_checksums` | R15, R17 |
| shutdown: `shutdown_aborts_a_stuck_connection_after_the_two_second_grace` | R22 (ends in [1.9 s, 6 s)) |
| shutdown: `shutdown_returns_early_when_in_flight_connections_finish` | R22 |
| shutdown: `shutdown_with_nothing_in_flight_is_prompt` | R22 |
| shutdown: `shutdown_ends_connections_that_are_still_connecting` | R22 |

Every test body runs under a watchdog (`within`), so a hang fails the test with a message instead of stalling the run. Timing in the health tests: cooldown checks that must land inside a 1 s cooldown take a few milliseconds, and the tests sleep 1.3 s to pass the cooldown.

## Acceptance
From `crates/ex08-tcp-reverse-proxy`:
```
cargo test --no-fail-fast
cargo test --release
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```
All four exit with code 0, and the two test runs report 27 passed, 0 failed (the whole suite takes about 5 s). Run `cargo test` 20 times in a row; every run must pass. There are no `#[ignore]` tests and no wall-clock performance thresholds. Record in your notes how long the 64 MiB test takes in `--release`.

## Extra tests to write
1. **Trial abort.** Unit-test your health table directly (the proxy cannot be restarted after `shutdown`): claim a trial, drop the claim without reporting, and check the backend is ejected for a fresh cooldown and can be trialled again after it. Catches an aborted trial that leaves the in-flight flag set (R11).
2. **Concurrent failures.** 50 simultaneous connections to a single refused backend with `eject_after_failures = 3`. Expect `backend_connect_failures` between 3 and 50 and `no_backend_available == 50`, then no new attempts during the cooldown. Catches a counter updated without a lock.
3. **Failure counter after ejection.** A connection that started before ejection fails after it. The cooldown must not be extended or the counter re-armed by that stale failure. Catches an ejection check written as `>=` instead of `==`.
4. **EOF without data.** Client connects and shuts down immediately; backend echoes. Expect `completed == 1` and both byte counters 0. Catches a forwarding loop that treats a zero-byte read as an error.
5. **Backend reset mid-stream.** The backend closes with unread data (RST). The client connection must end and `completed` must still increment. Catches an error path that skips the counter.
6. **All ejected.** Eject every backend, then connect. Expect `no_backend_available` up by 1 and no new attempts. Catches a fallback that tries ejected backends when nothing else is left.
7. **Many short connections.** 5,000 sequential connections, then check the process's task count or RSS did not grow with them (R23). Catches a `JoinSet` or `Vec<JoinHandle>` that is never reaped.
8. **Slow client.** A client that stops reading for 2 s while the backend sends 16 MiB. Expect no loss and no unbounded buffering in the proxy. Catches a copy loop that reads ahead without waiting for the write.

## Stretch (optional)
1. Weighted round-robin: `weights: Vec<u32>` in the config, with a smooth interleaving (no run of one backend longer than needed). Prove the distribution over `sum(weights)` connections is exact.
2. Least-connections selection with the same failover and ejection rules. Decide, and write down, how ties break and how R1 changes.
3. Limit concurrent connections per client IP using the structure from exercise 01, and count rejections in a new stat. Add active health checks (periodic connect probes) that feed the same ejection state. For TLS, note that a layer-4 proxy forwards the encrypted bytes untouched (TLS passthrough): it cannot route by hostname unless it peeks at the SNI field in the ClientHello, which conflicts with R14.
