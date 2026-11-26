# Exercise 06: Thread pool

**Time:** 2.5 h | **Crate:** `crates/ex06-thread-pool`

## Read first
**Rust Book**
- [Ch. 9.1: Unrecoverable Errors with panic!](https://doc.rust-lang.org/book/ch09-01-unrecoverable-errors-with-panic.html)
- [Ch. 15.3: Running Code on Cleanup with the Drop Trait](https://doc.rust-lang.org/book/ch15-03-drop.html)
- [Ch. 16.1: Using Threads to Run Code Simultaneously](https://doc.rust-lang.org/book/ch16-01-threads.html)
- [Ch. 16.2: Transfer Data Between Threads with Message Passing](https://doc.rust-lang.org/book/ch16-02-message-passing.html)
- [Ch. 16.3: Shared-State Concurrency](https://doc.rust-lang.org/book/ch16-03-shared-state.html)
- [Ch. 16.4: Extensible Concurrency with Send and Sync](https://doc.rust-lang.org/book/ch16-04-extensible-concurrency-sync-and-send.html)
- [Ch. 21.2: From Single-Threaded to Multithreaded Server](https://doc.rust-lang.org/book/ch21-02-multithreaded.html)
- [Ch. 21.3: Graceful Shutdown and Cleanup](https://doc.rust-lang.org/book/ch21-03-graceful-shutdown-and-cleanup.html)

**Standard library**
- [`catch_unwind`](https://doc.rust-lang.org/std/panic/fn.catch_unwind.html)

**Background**
- [Thread pool (Wikipedia)](https://en.wikipedia.org/wiki/Thread_pool): what a pool of worker threads fed by a task queue is, and why it limits load.
- [Applying Back Pressure When Overloaded (Mechanical Sympathy)](https://mechanical-sympathy.blogspot.com/2012/05/apply-back-pressure-when-overloaded.html): why the queue in front of a pool must be bounded and what a full queue should do to the producer.

## Task
Build a thread pool that runs submitted closures on a fixed number of worker threads. Accepted jobs wait in a
queue of fixed capacity. A job that panics must not stop its worker. Use only `std`.

## API
A job is *waiting* from the moment a submission is accepted until a worker starts it. Only waiting jobs count
against `queue_capacity`. No call may panic, except that `new` may panic if the OS cannot start a thread.
`ThreadPool` is `Send + Sync`, so many threads can submit through an `Arc<ThreadPool>`.

### `PoolError` and `TryExecuteError`
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolError { ZeroWorkers, ZeroCapacity, Shutdown }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TryExecuteError { Full, Shutdown }
```

### `ThreadPool::new`
```rust
pub fn new(workers: usize, queue_capacity: usize) -> Result<ThreadPool, PoolError>
```
| Input | Returns |
|---|---|
| `workers == 0` (any `queue_capacity`) | `Err(PoolError::ZeroWorkers)` |
| `workers >= 1` and `queue_capacity == 0` | `Err(PoolError::ZeroCapacity)` |
| otherwise | `Ok` |

On `Ok`, the process has exactly `workers` more threads than before the call. Once the pool is dropped, the
thread count is back to the value before `new`. (The test counts entries in `/proc/self/task`, so it runs on
Linux only.)

### `ThreadPool::execute`
```rust
pub fn execute<F: FnOnce() + Send + 'static>(&self, f: F) -> Result<(), PoolError>
```
1. If the pool is closed, returns `Err(PoolError::Shutdown)` and does not run `f`.
2. If `queue_capacity` jobs are waiting, blocks until a worker starts one. Then it accepts `f` and returns
   `Ok(())`.
3. Otherwise returns `Ok(())` without waiting for `f` to run.
4. Every job accepted with `Ok(())` runs exactly once on a pool thread, even if the pool is closed, shut down or
   dropped right afterwards.
5. With `workers >= 2`, two jobs can run at the same time. Two jobs that each wait for the other to start both
   finish.
6. With `workers == 1`, jobs run in the order they were accepted.

### `ThreadPool::try_execute`
```rust
pub fn try_execute<F: FnOnce() + Send + 'static>(&self, f: F) -> Result<(), TryExecuteError>
```
Never blocks. Rules 4 to 6 of `execute` apply to accepted jobs.

| Situation | Returns |
|---|---|
| pool is closed (also when the queue is full) | `Err(TryExecuteError::Shutdown)` |
| `queue_capacity` jobs are waiting | `Err(TryExecuteError::Full)` |
| otherwise | `Ok(())` |

With every worker inside a running job, exactly `queue_capacity` calls return `Ok(())`. The next returns
`Err(TryExecuteError::Full)`.

### `ThreadPool::close`
```rust
pub fn close(&self)
```
1. Returns at once, even while jobs are running. It does not wait for any job.
2. After it returns, `execute` returns `Err(PoolError::Shutdown)` and `try_execute` returns
   `Err(TryExecuteError::Shutdown)`, on every thread.
3. Jobs accepted before it still run.
4. Calling it again has no effect.

### `ThreadPool::shutdown`
```rust
pub fn shutdown(self)
```
Closes the pool. Returns after every accepted job has run and every worker thread has exited. Dropping the pool
without calling `shutdown` does the same.

### `ThreadPool::panicked_jobs`
```rust
pub fn panicked_jobs(&self) -> usize
```
1. A job that panics does not stop its worker. Later jobs still run.
2. Counts each job that panicked once. A job that returns normally is not counted.
3. Once all submitted jobs have finished, it equals the number of jobs that panicked.

## Example
`ThreadPool::new(2, 3)`. Both workers are inside jobs that wait for a signal, so no job is waiting.

| Call | Returns |
|---|---|
| `try_execute(\|\| {})` | `Ok(())` |
| `try_execute(\|\| {})` | `Ok(())` |
| `try_execute(\|\| {})` | `Ok(())` (3 jobs waiting) |
| `try_execute(\|\| {})` | `Err(TryExecuteError::Full)` |
| `execute(\|\| {})` | blocks until a worker finishes its job and starts a waiting one, then `Ok(())` |
| `close()` | returns at once |
| `try_execute(\|\| {})` | `Err(TryExecuteError::Shutdown)` |
| `execute(\|\| {})` | `Err(PoolError::Shutdown)` |
| `shutdown()` | returns after all accepted jobs have run |

`ThreadPool::new(0, 0)` returns `Err(PoolError::ZeroWorkers)`. `ThreadPool::new(4, 0)` returns
`Err(PoolError::ZeroCapacity)`.

## Done when
Edit only `src/lib.rs`. Do not edit `tests/` or `Cargo.toml`, and add no dependencies. The starter compiles, and
15 of its 16 tests fail until you finish.

From `crates/ex06-thread-pool`, all of these exit 0:
```
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```
`cargo test` reports 15 passed in `tests/pool.rs` and 1 passed in `tests/threads.rs`. Run it 20 times in a row;
all 20 must pass.

## Extra tests (optional)
- `new(1, 1)`, 1,000 jobs from 4 producer threads: all 1,000 run.
- `close()` twice, then `shutdown()`: no panic, no hang.
- A job calls `try_execute` on its own full pool: returns `Err(TryExecuteError::Full)`, no deadlock.
- A job calls `std::panic::panic_any(42)`: `panicked_jobs()` becomes 1.
- A closure holding an `Arc` clone is rejected with `Shutdown`: `Arc::strong_count` is 1 afterwards.

## Stretch (optional)
- A producer blocked in `execute` returns `Err(PoolError::Shutdown)` as soon as `close` is called.
- Add `pool.scope(|s| s.spawn(|| ...))`, which borrows non-`'static` data and blocks until all spawned jobs finish.
