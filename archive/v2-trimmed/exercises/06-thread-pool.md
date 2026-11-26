# Exercise 06: Bounded thread pool with graceful shutdown

**Time:** ~2.5 h | **Area:** concurrency | **Crate:** `crates/ex06-thread-pool`
**Builds on:** none (standalone)

## Read first
- [Rust Book 15.3: Drop](https://doc.rust-lang.org/book/ch15-03-drop.html)
- [Rust Book 15.4: Rc](https://doc.rust-lang.org/book/ch15-04-rc.html) (the single-threaded counterpart of `Arc`)
- [Rust Book 16.1: Threads](https://doc.rust-lang.org/book/ch16-01-threads.html)
- [Rust Book 16.2: Message passing](https://doc.rust-lang.org/book/ch16-02-message-passing.html)
- [Rust Book 16.3: Shared state](https://doc.rust-lang.org/book/ch16-03-shared-state.html)
- [Rust Book 16.4: Sync and Send](https://doc.rust-lang.org/book/ch16-04-extensible-concurrency-sync-and-send.html)
- [Rust Book 21.1: A single-threaded server](https://doc.rust-lang.org/book/ch21-01-single-threaded.html)
- [Rust Book 21.2: A multithreaded server](https://doc.rust-lang.org/book/ch21-02-multithreaded.html). Its pool is the starting design. It has an unbounded queue and no panic handling.
- [Rust Book 21.3: Graceful shutdown and cleanup](https://doc.rust-lang.org/book/ch21-03-graceful-shutdown-and-cleanup.html)
- std docs: [`sync_channel`](https://doc.rust-lang.org/std/sync/mpsc/fn.sync_channel.html), [`SyncSender::try_send`](https://doc.rust-lang.org/std/sync/mpsc/struct.SyncSender.html#method.try_send), [`catch_unwind`](https://doc.rust-lang.org/std/panic/fn.catch_unwind.html)

## What you are building
A worker pool runs submitted closures on a fixed set of threads. Servers use one so that load cannot create unlimited threads. You extend the Book's pool with a bounded queue (backpressure), a non-blocking submit, panic isolation, and a shutdown that finishes all accepted work before joining the workers.

## Concepts
**Backpressure.** If producers submit faster than workers finish, an unbounded queue grows until memory runs out. A bounded queue pushes the problem back to the producers: when it is full, `execute` blocks, so a producer slows to the speed of the workers. `try_execute` gives the producer the choice to drop or retry instead of waiting.

```
 producers            bounded queue (capacity 3)         workers
 ---------            --------------------------         -------
 P1 --execute-->  +-----+-----+-----+  recv   --> W1 [running job A]
 P2 --execute-->  | j4  | j5  | j6  |  ------>    W2 [running job B]
 P3 --try_exec->  +-----+-----+-----+  ------>    W3 [running job C]
                  full: P1, P2 block
                        P3 gets Err(Full)
```

Jobs that a worker has taken (A, B, C) no longer count against the capacity.

**Worked example.** `ThreadPool::new(2, 3)`, and both workers are stuck inside jobs.
1. `try_execute` three times returns `Ok` three times. The queue holds 3 jobs.
2. A fourth `try_execute` returns `Err(Full)`. A fourth `execute` would block.
3. One worker finishes, takes job 1, and a slot frees. The blocked `execute` now returns `Ok`.

**Graceful shutdown.** Workers loop on `recv()`. `recv` returns jobs until the queue is empty and every sender is dropped, then it returns `Err`. So dropping the sender is the shutdown signal, and the workers drain the queue before they see it. `shutdown` drops the sender, then joins each worker.

**Panic isolation.** A panic in a job unwinds the worker thread and kills it. `catch_unwind` around the job stops the unwind at the job boundary, so the worker takes the next job.

## Milestones
Run each command from `crates/ex06-thread-pool`. Names after `--` are filters, so only matching tests run.

1. **Construction and run (R1, R2, R10).** `new`, `execute`, `shutdown`, `Drop`. Green: `new_rejects_zero_arguments`, `ten_thousand_jobs_counted_exactly`, `drop_without_shutdown_drains`, `dropping_the_pool_joins_every_worker`. Run `cargo test -- new_rejects ten_thousand drop_ dropping` (4 tests).
2. **Backpressure (R3, R4).** Bounded queue and `try_execute`. Green: `try_execute_reports_full_when_workers_busy_and_queue_at_capacity`, `blocked_execute_resumes_when_a_slot_frees`. Run `cargo test -- blocked_execute try_execute_reports` (2 tests).
3. **Graceful shutdown (R5, R6).** `close` and the drain. Green: `submissions_after_close_return_shutdown`, `shutdown_runs_every_queued_job`, `stress_drain_200_times`. Run `cargo test -- shutdown submissions stress` (3 tests).
4. **Panics (R7).** Green: `panicking_jobs_do_not_kill_workers`, `randomized_mixes_of_jobs_panics_and_submission_styles`. Run `cargo test -- panick randomized` (2 tests).
5. **Ordering and concurrency (R8, R9, R11).** Green: `one_worker_runs_jobs_in_fifo_order`, `eight_producers_submit_concurrently`. Then run `cargo test` for all 13 and the grep in Acceptance.

## Starting point
- Edit: `crates/ex06-thread-pool/src/lib.rs`. All bodies are `todo!()`. Add private fields and types as needed.
- Do not edit: `tests/`, `Cargo.toml`. The crate has no dependencies, and you may not add any.
- Run: `cd crates/ex06-thread-pool && cargo test`.
- `close(&self)` is part of the fixed API. `shutdown(self)` consumes the pool, so without `close` no test could observe the pool refusing work while other threads still share it through an `Arc`.
- `std::sync::mpsc` is allowed. `crossbeam`, `rayon` and `flume` are not.

## Requirements
1. `ThreadPool::new(workers, queue_capacity)` starts exactly `workers` OS threads. It returns `Err(PoolError::ZeroWorkers)` if `workers == 0`, else `Err(PoolError::ZeroCapacity)` if `queue_capacity == 0`. With both zero the result is `ZeroWorkers`.
2. `execute(f)` queues `f` and returns `Ok(())`. If `queue_capacity` jobs are already waiting, it blocks until a slot frees.
3. The queue holds at most `queue_capacity` waiting jobs. A job a worker has taken does not count.
4. `try_execute(f)` never blocks. It returns `Err(TryExecuteError::Full)` when `queue_capacity` jobs are waiting.
5. After `close()`, `execute` returns `Err(PoolError::Shutdown)` and `try_execute` returns `Err(TryExecuteError::Shutdown)`. `Shutdown` takes precedence over `Full`. `close` is idempotent and does not wait.
6. `shutdown(self)` closes the pool, runs every job for which `execute` or `try_execute` returned `Ok`, and joins all workers before returning. `Drop` does the same when the pool is dropped without `shutdown`.
7. A job that panics is caught. `panicked_jobs()` increases by 1 per panicked job and the worker keeps taking jobs. A job that returns normally does not change it.
8. With one worker, jobs run in the order they were accepted.
9. `ThreadPool` is `Send + Sync`, so `Arc<ThreadPool>` works across producer threads.
10. No leaked threads: after the pool is dropped, the process thread count equals the count before `new`.
11. A worker must not hold any lock while it runs a job. Two workers must be able to run jobs at the same time.
12. `src/` must not mention `crossbeam`, `rayon` or `flume`.

## Common mistakes
- Writing `while let Ok(job) = rx.lock().unwrap().recv() { job() }`. The guard lives for the whole loop body, so only one worker runs at a time. Take the job in a separate `let` statement so the guard drops first. R11 and `try_execute_reports_...` catch it.
- Keeping the sender in a plain `Option<SyncSender<_>>` field. `close(&self)` cannot take it. Put it behind a `Mutex`.
- Holding that `Mutex` across a blocking `send`. A full queue then blocks `close` and every other producer. Clone the sender, release the lock, then send.
- Forgetting to drop the sender before joining workers in `Drop`. The workers wait on `recv` forever and `join` hangs.
- Letting a panic escape the job. The worker thread dies, and with one worker the pool stops serving.
- Using `queue_capacity = 0`. `sync_channel(0)` is a rendezvous channel, which has no waiting slots. The pool rejects it.

## Given tests
13 tests run by default: 12 in `tests/pool.rs`, 1 in `tests/threads.rs`. Each body in `pool.rs` runs under a 10 s watchdog, so a deadlock fails instead of hanging.

| Test | Checks |
|---|---|
| `new_rejects_zero_arguments` | R1 |
| `ten_thousand_jobs_counted_exactly` | R2, R6 (10,000 jobs, 4 workers, capacity 64) |
| `try_execute_reports_full_when_workers_busy_and_queue_at_capacity` | R3, R4, R11 |
| `blocked_execute_resumes_when_a_slot_frees` | R2 |
| `shutdown_runs_every_queued_job` | R5, R6 (50 jobs queued behind a gated job) |
| `drop_without_shutdown_drains` | R6 |
| `submissions_after_close_return_shutdown` | R5, R9 |
| `panicking_jobs_do_not_kill_workers` | R7 (100 panics, then 100 good jobs) |
| `eight_producers_submit_concurrently` | R2, R9 (8 x 1,000 jobs) |
| `one_worker_runs_jobs_in_fifo_order` | R8 |
| `stress_drain_200_times` | R6 repeated 200 times |
| `randomized_mixes_of_jobs_panics_and_submission_styles` | R2, R4, R7 (40 seeded cases) |
| `tests/threads.rs`: `dropping_the_pool_joins_every_worker` | R1, R10 (Linux only) |

## Acceptance
1. `cd crates/ex06-thread-pool && cargo test --no-fail-fast` exits 0 with `12 passed` in `pool` and `1 passed` in `threads`. Run it 20 times in a row; all 20 must pass.
2. `cargo test --release` exits 0.
3. `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` exit 0.
4. `grep -rniE 'crossbeam|rayon|flume' src/` prints nothing.

## Extra tests to write
1. `queue_capacity = 1`, one worker, 1,000 jobs from 4 producers. Catches a stall when a single slot cycles.
2. Call `close()` twice, then `shutdown()`. Catches a panic or double join on the second close.
3. From inside a running job, call `try_execute` on a full queue. It must return `Full`, not deadlock. Catches a lock held while running a job.
4. A job that panics with a non-string payload (`std::panic::panic_any(42)`). It must be counted. Catches payload downcasting that unwraps.
5. Keep `workers = 4`, drop the pool while 4 jobs sleep 50 ms each. Drop must return only after all 4 finish. Catches joining before draining.
6. 8 producers blocked on a full queue while another thread calls `close()`. Each returns `Ok` or `Shutdown`, and the number of jobs that run equals the number of `Ok` results. Catches a lost or duplicated job.
7. Put an `Arc` clone in a job rejected with `Shutdown`. Afterwards `Arc::strong_count` is 1. Catches a leak of rejected jobs.

## Stretch (optional)
1. Make a producer blocked in `execute` return `Shutdown` when `close` is called, instead of waiting for a slot. Replace the channel with a `Mutex<VecDeque<Job>>` and two `Condvar`s.
2. Add `queued()` and `completed_jobs()` counters and assert that `queued() <= queue_capacity` while 4 producers run.
3. A `scope`-style API, `pool.scope(|s| s.spawn(|| ...))`, that borrows non-`'static` data and blocks until all spawned jobs finish. Decide what happens if a scoped job panics.
