# Exercise 04: Bounded thread pool with graceful shutdown

**Time:** ~3 h | **Area:** concurrency | **Crate:** `crates/ex04-thread-pool`
**Builds on:** none (standalone)

## Read first
- [Rust Book 16.1: Threads](https://doc.rust-lang.org/book/ch16-01-threads.html)
- [Rust Book 16.2: Message passing](https://doc.rust-lang.org/book/ch16-02-message-passing.html) (for contrast: you may not use channels here)
- [Rust Book 16.3: Shared state](https://doc.rust-lang.org/book/ch16-03-shared-state.html)
- [Rust Book 16.4: Sync and Send](https://doc.rust-lang.org/book/ch16-04-extensible-concurrency-sync-and-send.html)
- [Rust Book 21.2: A multithreaded server](https://doc.rust-lang.org/book/ch21-02-multithreaded.html). You must go beyond it: its pool has an unbounded queue and no panic handling.
- [Rust Book 15.3: Drop](https://doc.rust-lang.org/book/ch15-03-drop.html) and [15.4: Rc](https://doc.rust-lang.org/book/ch15-04-rc.html) (the `Arc` counterpart)
- std docs: [`Mutex`](https://doc.rust-lang.org/std/sync/struct.Mutex.html), [`Condvar`](https://doc.rust-lang.org/std/sync/struct.Condvar.html), [`Arc`](https://doc.rust-lang.org/std/sync/struct.Arc.html), [`JoinHandle`](https://doc.rust-lang.org/std/thread/struct.JoinHandle.html), [`catch_unwind`](https://doc.rust-lang.org/std/panic/fn.catch_unwind.html), [`VecDeque`](https://doc.rust-lang.org/std/collections/struct.VecDeque.html)

## What you are building
A worker pool runs submitted closures on a fixed set of threads. Servers use one so that load cannot create unlimited threads. A bounded queue adds backpressure: when workers fall behind, producers block (or are told "full") instead of growing memory without limit. Graceful shutdown stops intake but finishes work already accepted. A job that panics must not take its worker down.

You build the queue yourself from `Mutex<VecDeque<Job>>` and two `Condvar`s.

## Starting point
- Edit: `crates/ex04-thread-pool/src/lib.rs`. All bodies are `todo!()`. Add private fields and types as needed.
- Do not edit: `tests/`, `Cargo.toml`. The crate has no dependencies, and you may not add any.
- Run: `cd crates/ex04-thread-pool && cargo test`.
- The API is fixed, with one addition: `close(&self)`. `shutdown(self)` consumes the pool, so without `close` no test could observe the pool refusing work while other threads still hold it through an `Arc`.

## Requirements
1. `ThreadPool::new(workers, queue_capacity)` starts exactly `workers` OS threads. It returns `Err(PoolError::ZeroWorkers)` if `workers == 0`, else `Err(PoolError::ZeroCapacity)` if `queue_capacity == 0`. With both zero the result is `ZeroWorkers`.
2. The queue holds at most `queue_capacity` jobs. A job a worker has taken out of the queue no longer counts. `queued()` returns the number of jobs waiting.
3. `execute(f)` appends `f` and returns `Ok(())`. If the queue is full it blocks until a slot frees. It never spins: a blocked producer waits on a `Condvar`.
4. `try_execute(f)` never blocks. It returns `Err(TryExecuteError::Full)` when `queued() == queue_capacity`.
5. Once shutdown has begun, `execute` returns `Err(PoolError::Shutdown)` and `try_execute` returns `Err(TryExecuteError::Shutdown)`. `Shutdown` takes precedence over `Full`. A producer blocked in `execute` when shutdown begins wakes and returns `Shutdown`; its closure is never run.
6. `close(&self)` begins shutdown without joining anything. It is idempotent and wakes all blocked producers and idle workers.
7. `shutdown(self)` calls the equivalent of `close`, runs every job that was already queued, and joins all workers before returning. Workers exit only when the queue is empty and the pool is closed.
8. `Drop` behaves like `shutdown` when the pool is dropped without it. If the last `Arc<ThreadPool>` is released by a job on a worker thread, `Drop` must not join the current thread.
9. A job that panics is caught with `catch_unwind`. `panicked_jobs()` increases by 1 and the worker keeps taking jobs. `completed_jobs()` counts jobs that returned normally and does not count panicked ones.
10. With one worker, jobs run in the order they were accepted (FIFO). With several workers, jobs start in FIFO order but may finish in any order.
11. `ThreadPool` is `Send + Sync`, so `Arc<ThreadPool>` works across producer threads.
12. No leaked threads: after the pool is dropped, the process thread count equals the count before `new`.
13. `src/` must not mention `mpsc`, `crossbeam`, `rayon`, `flume` or `async_channel`.
14. Never run a job while holding the queue lock.

## Given tests
All in `tests/pool.rs` unless noted. Each body runs under a 10 s watchdog, so a deadlock fails instead of hanging.

| Test | Checks |
|---|---|
| `new_rejects_zero_arguments` | R1 |
| `ten_thousand_jobs_counted_exactly` | R3, R7 (10,000 jobs, 4 workers, capacity 64) |
| `try_execute_reports_full_when_workers_busy_and_queue_at_capacity` | R2, R4 |
| `blocked_execute_resumes_when_a_slot_frees` | R3 |
| `shutdown_runs_every_queued_job` | R7 (50 jobs queued behind a gated job) |
| `shutdown_called_from_another_thread_drains_too` | R7, R11 |
| `drop_without_shutdown_drains` | R8 |
| `submissions_after_close_return_shutdown` | R5, R6 |
| `close_wakes_a_blocked_producer_with_shutdown` | R5, R6 |
| `panicking_jobs_do_not_kill_workers` | R9 (100 panics, then 100 good jobs) |
| `single_worker_with_all_workers_panicking_still_serves` | R9 |
| `eight_producers_submit_concurrently` | R3, R11 (8 x 1,000 jobs) |
| `one_worker_runs_jobs_in_fifo_order` | R10 |
| `queue_never_exceeds_capacity` | R2 |
| `stress_drain_200_times` | R7 repeated 200 times |
| `randomized_mixes_of_jobs_panics_and_submission_styles` | R3, R4, R9 (40 seeded cases) |
| `tests/threads.rs`: `dropping_the_pool_joins_every_worker` | R1, R12 (Linux only) |
| `throughput_one_million_empty_jobs` (ignored) | measurement only |

That is 17 given tests that run by default, plus 1 ignored.

## Acceptance
1. `cd crates/ex04-thread-pool && cargo test --no-fail-fast` exits 0 with `16 passed` in `pool` and `1 passed` in `threads`. Run it 20 times in a row; all 20 must pass.
2. `cargo test --release` exits 0.
3. `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` exit 0.
4. `grep -rniE 'mpsc|crossbeam|rayon|flume|async_channel' src/` prints nothing.
5. `cargo test --release -- --ignored --nocapture` passes. Record the printed jobs/s in your notes. The reference solution measured about 400,000 jobs/s (4 workers, capacity 1024, release build). No threshold is enforced.

## Extra tests to write
1. `queue_capacity = 1`, one worker, 1,000 jobs from 4 producers. Catches a lost wakeup when only one slot cycles.
2. Call `close()` twice, then `shutdown()`. Catches a double-join or a panic on the second close.
3. Submit from inside a running job with `try_execute` on a full queue. It must return `Full`, not deadlock. Catches holding the queue lock while running a job.
4. 8 producers blocked on a full queue, then `close()`. All 8 must return `Shutdown`. Catches `notify_one` where `notify_all` is needed.
5. A job that holds the last `Arc<ThreadPool>` and drops it. The process must not hang. Catches the self-join in R8.
6. A job that panics with a non-string payload (`std::panic::panic_any(42)`). It must be counted. Catches payload downcasting that unwraps.
7. Keep `workers = 4`, drop the pool while 4 jobs sleep 50 ms each. Drop must return only after all 4 finish. Catches joining before draining.
8. Check that dropped closures are released: put an `Arc` clone in a job rejected with `Shutdown` and assert `Arc::strong_count` is 1 afterwards. Catches a leak of rejected jobs.

## Stretch (optional)
1. Work stealing: give each worker its own deque and let an idle worker take from the back of another's. Keep the global bound on queued jobs. Compare `jobs/s` with your single-queue pool.
2. A `scope`-style API, `pool.scope(|s| s.spawn(|| ...))`, that allows borrowing non-`'static` data and blocks until all spawned jobs finish. Think about what happens if a scoped job panics.
3. Add `rayon` as a dev-dependency in a separate benchmark and record the throughput of both on 1,000,000 empty jobs. `rayon` is allowed only outside `src/`.
