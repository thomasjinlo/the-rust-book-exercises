# Exercise 06: Thread pool

**Time:** 2 h | **Crate:** `crates/ex06-thread-pool`

## Task
Build a pool of worker threads. Callers hand it closures, and the workers run them. Use only `std`.

## Example
`ThreadPool::new(1)`, then these calls:

| Call | Effect |
|---|---|
| `execute(\|\| println!("a"))` | returns at once |
| `execute(\|\| println!("b"))` | returns at once |
| `execute(\|\| panic!("oops"))` | returns at once. The panic does not stop the worker |
| `execute(\|\| println!("c"))` | returns at once |
| `shutdown()` | returns after the output `a`, `b`, `c` (in this order) |

With `ThreadPool::new(4)`, four jobs can run at the same time.

## API
Create the pool with `ThreadPool::new(workers: usize)`, where `workers >= 1`. There is no error type.

### `execute`
```rust
pub fn execute<F: FnOnce() + Send + 'static>(&self, f: F)
```
1. Hands `f` to the pool and returns without waiting for it to run.
2. Every job runs exactly once. With 1 worker, jobs run in the order they were submitted.
3. With 2 or more workers, jobs run at the same time.
4. A job that panics does not stop its worker. Later jobs still run.

### `shutdown`
```rust
pub fn shutdown(self)
```
1. Returns after every job submitted so far has run and all workers have stopped.
2. Dropping the pool does the same.

## Done when
Edit only `src/lib.rs`. From `crates/ex06-thread-pool`, `cargo test` reports 8 passed (the starter fails all 8).
Hard rule: 4 workers and 4 jobs that each wait on a shared `Barrier::new(4)` must all finish, which only happens
if the jobs really run in parallel. The test gives up after 10 seconds.

## Read first
- [Ch. 16.2: Transfer Data Between Threads with Message Passing](https://doc.rust-lang.org/book/ch16-02-message-passing.html)
- [Ch. 16.3: Shared-State Concurrency](https://doc.rust-lang.org/book/ch16-03-shared-state.html)
- [Thread pool (Wikipedia)](https://en.wikipedia.org/wiki/Thread_pool)

## Stretch (optional)
- Bound the queue and add `try_execute`, which returns an error when the queue is full.
- Count the jobs that panicked.
