# Exercise 05: Lock-free SPSC ring buffer

**Time:** ~5 h | **Area:** concurrency | **Crate:** `crates/ex05-spsc-ring`
**Builds on:** none (standalone)

## Read first
- [The Rust Book, ch16.4: Extensible Concurrency with `Sync` and `Send`](https://doc.rust-lang.org/book/ch16-04-extensible-concurrency-sync-and-send.html)
- [The Rust Book, ch20.1: Unsafe Rust](https://doc.rust-lang.org/book/ch20-01-unsafe-rust.html)
- [`std::sync::atomic`](https://doc.rust-lang.org/std/sync/atomic/index.html) and [`Ordering`](https://doc.rust-lang.org/std/sync/atomic/enum.Ordering.html)
- [`MaybeUninit`](https://doc.rust-lang.org/std/mem/union.MaybeUninit.html), [`UnsafeCell`](https://doc.rust-lang.org/std/cell/struct.UnsafeCell.html), [`Arc`](https://doc.rust-lang.org/std/sync/struct.Arc.html)
- Rustonomicon: [Atomics](https://doc.rust-lang.org/nomicon/atomics.html) and [Send and Sync](https://doc.rust-lang.org/nomicon/send-and-sync.html)
- Optional: [Rust Atomics and Locks](https://mara.nl/atomics/), chapters on memory ordering and building a channel

## What you are building
A bounded single-producer single-consumer (SPSC) queue over a power-of-two ring buffer.
Queues like this connect a network receive thread to a worker, an audio callback to a UI thread, or a logger to its writer, wherever a mutex or a syscall on the hot path is too slow.
The producer and consumer each own one atomic index. Neither ever waits for the other, so both operations are wait-free: a fixed number of steps, no loops, no retries.
You write the `unsafe` core: `MaybeUninit` slots, index masking, Acquire/Release hand-off, and drop of leftover items.

## Starting point
Edit only `src/lib.rs`. Do not edit `tests/spsc.rs` (add your own tests in a new file, see below).

Provided, with no `todo!()`:
- `CachePadded<T>`, a `#[repr(align(64))]` wrapper. Wrap each index in it so the two indices sit on different cache lines.
- `Shared<T>` with its fields, and `Producer<T>` / `Consumer<T>` with their fields.
- `unsafe impl Send/Sync for Shared<T>`, so the given tests compile on the starter. You replace the placeholder comment with a real safety argument (R9).
- Two `compile_fail` doctests proving neither half is `Sync`. They pass on the starter and must keep passing.

`Producer` and `Consumer` carry a `PhantomData<Cell<()>>` field. Without it, `Arc<Shared<T>>` is `Sync` and both halves would be too. Do not remove it.

Run: `cd crates/ex05-spsc-ring && cargo test`

## Requirements
Let `cap` be the capacity. Indices are `usize` and only ever increase with wrapping arithmetic. Slot `i` is `buf[i & mask]`.

- **R1** `channel::<T>(n)` returns capacity `n.next_power_of_two()`, except `n == 0` returns 1. It panics if the rounded value overflows `usize`. Examples: 0→1, 1→1, 2→2, 3→4, 5→8, 64→64, 65→128, 1000→1024.
- **R2** `Producer::capacity()` and `Consumer::capacity()` return the same `cap`.
- **R3** `pop` returns items in the exact order they were pushed (FIFO). `pop` on an empty queue returns `None`.
- **R4** `push` on a queue holding exactly `cap` items returns `Err(value)` with the original value, and does not change the queue. After one `pop`, `push` succeeds again.
- **R5** The queue works across index wraparound: any sequence of at least `10 * cap` pushes and pops preserves FIFO and the full/empty rules. Use `wrapping_add` and `wrapping_sub` so correctness does not depend on `usize` overflow never happening. Do not compare `tail > head` to detect fullness or emptiness.
- **R6** Memory ordering. The producer reads its own `tail` with `Relaxed`, reads `head` with `Acquire`, and publishes with a `Release` store of `tail`. The consumer mirrors this. Do not use `SeqCst`. Do not use `compare_exchange`, `fetch_add` or any loop in `push`/`pop`.
- **R7** `Producer::is_full` is `true` iff the queue holds `cap` items at the instant `head` is read. `Consumer::is_empty` is `true` iff `head == tail` at the instant `tail` is read. A `false` result from `is_empty` guarantees the next `pop` returns `Some`. A `false` result from `is_full` guarantees the next `push` succeeds.
- **R8** `Shared<T>` implements `Drop`. It drops every item in `head..tail` exactly once, in FIFO order, whichever half is dropped first. Items already popped are not dropped again. Items returned by a failed `push` are not dropped by the queue.
- **R9** `Producer<T>` and `Consumer<T>` are `Send` when `T: Send` and never `Sync`. The `unsafe impl` for `Shared<T>` carries a `// SAFETY:` comment of 4 to 6 lines that names: which thread touches a slot in which state, which two atomic operations hand a slot over, and why `T: Send` is required.
- **R10** Every `unsafe` block has a `// SAFETY:` comment naming the invariant it relies on.
- **R11** Zero-sized `T` (for example `()`) works with the same capacity and full/empty behavior.
- **R12** `push` and `pop` perform no heap allocation. Only `channel` allocates.

## Given tests
All in `tests/spsc.rs`. 18 run by default, 1 is `#[ignore]`. The two doctests in `src/lib.rs` add 2 more.

| Test | Requirement |
|---|---|
| `capacity_rounding` | R1, R2 (10 input/output pairs) |
| `fifo_single_thread` | R3, R7 |
| `full_returns_value` | R4, R7 |
| `wraparound_ten_times_capacity` | R5 |
| `fill_drain_cycles_after_wrap` | R4, R5 (25 full/drain cycles at cap 4) |
| `drop_counts_popped_items` | R8 |
| `rejected_push_is_not_dropped_by_queue` | R4, R8 |
| `drop_order_producer_first` | R8 (leftovers straddle the buffer end) |
| `drop_order_consumer_first` | R8 |
| `drop_empty_and_full_queues` | R8 |
| `leftovers_dropped_once_across_threads` | R8 across threads |
| `two_thread_stress_500k` | R3, R6: 500,000 `u64` in strict order, cap 1024 |
| `two_thread_stress_capacity_2` | R3, R6: 200,000 `u64`, cap 2 |
| `two_thread_heap_payloads_counted` | R6, R8: `Box`, `String` and a counting-drop type, 100,000 items |
| `zero_sized_type` | R11 |
| `capacity_one_ping_pong_1m` | R1, R6: two cap-1 queues, 1,000,000 round trips |
| `randomized_model_check_single_thread` | R3, R4, R5, R7 against `VecDeque`, 20 seeds x 5,000 steps |
| `halves_are_send` | R9 (`Send` check) |
| `two_thread_stress_20m_release` (`#[ignore]`) | R6 throughput, prints M ops/sec |
| 2 `compile_fail` doctests | R9 (`!Sync`) |

Every threaded test runs under a watchdog thread, so a deadlock fails the test instead of hanging. On the unmodified starter, `cargo test` finishes in under 1 s with 17 failures.

## Acceptance
```
cd crates/ex05-spsc-ring
cargo test                              # exit 0: 18 passed, 1 ignored, plus 2 doctests passed
cargo test --release                    # exit 0, same counts
cargo test --release -- --ignored --nocapture   # exit 0, prints "spsc: X M ops/sec"
cargo clippy --all-targets -- -D warnings       # exit 0
cargo fmt --check                               # exit 0
```
The throughput number has no threshold. Record it in your notes. The reference solution measured 8.4 M ops/sec on a 2-core ARM VM that was also running other jobs. A desktop CPU is typically several times faster.

Also required: grep your `unsafe` blocks and confirm each has a `SAFETY:` comment.

### Optional: Miri
Miri executes the code in an interpreter that detects data races, use of uninitialized memory, double drops and leaks. It is slow, so skip the long tests:
```
rustup toolchain install nightly --component miri
cargo +nightly miri test --test spsc -- --skip randomized --skip two_thread_stress --skip ping_pong --skip heap_payloads
```
The reference solution passes 13 tests this way in about 3 s of test time. The suggested command `cargo +nightly miri test --lib` runs zero tests, because the given tests are in `tests/`. Move some of them into `src/lib.rs` under `#[cfg(test)]` if you want to use `--lib`.

## Extra tests to write
Put them in `tests/extra.rs`.
1. Push `cap` items, pop 1, push 1, repeated 10,000 times at cap 8. Catches a full check that is off by one only after wraparound.
2. A `Drop` type that panics on the second drop of the same id (track ids in a `Mutex<HashSet>`). Catches double drops that a plain counter misses when one item is dropped twice and another leaked.
3. Start both indices near `usize::MAX`. Add a `#[cfg(test)]` constructor in `src/lib.rs` that sets the initial `head` and `tail` to `usize::MAX - 3`, then push/pop 20 items at cap 4. Catches `+` and `-` where `wrapping_add` and `wrapping_sub` are needed (a debug-build overflow panic).
4. `Producer<T>` moved to another thread, producer dropped there while the consumer is still popping. Catches a Drop that frees the buffer when only one half goes away.
5. Heap payload (`Box<[u64; 8]>` filled with the item id), 1,000,000 items at cap 16, consumer verifies a checksum. Catches torn reads from a missing Acquire on a weakly ordered CPU (ARM). x86 hides this bug.
6. Compile-fail doctest: `Producer<std::rc::Rc<u8>>` is not `Send`. Catches a missing `T: Send` bound on the `unsafe impl`.
7. `channel::<u8>(usize::MAX)` panics (`#[should_panic]`). Catches silent wraparound of the capacity computation.
8. `size_of::<CachePadded<AtomicUsize>>() == 64` and `align_of == 64`. Catches a missing or lost `repr(align(64))`.

## Stretch (optional)
- **loom model test.** Add `loom = "0.7"` as a `[target.'cfg(loom)'.dependencies]` entry, swap `std::sync::atomic` and `Arc` for `loom::sync` under `#[cfg(loom)]`, and write:
  ```rust
  #[test]
  fn loom_two_items() {
      loom::model(|| {
          let (mut p, mut c) = channel::<u32>(2);
          let t = loom::thread::spawn(move || { p.push(1).unwrap(); p.push(2).unwrap(); });
          let mut got = vec![];
          while got.len() < 2 { if let Some(v) = c.pop() { got.push(v); } else { loom::thread::yield_now(); } }
          t.join().unwrap();
          assert_eq!(got, [1, 2]);
      });
  }
  ```
  Run with `RUSTFLAGS="--cfg loom" cargo test --release loom`. Then change the `Release` store in `push` to `Relaxed` and confirm loom reports a failure. Plain tests on x86 do not catch that change. I did not run loom for this exercise.
- **Batch API.** `push_slice(&mut self, &[T]) -> usize` for `T: Copy` and `pop_into(&mut self, &mut [T]) -> usize`, with one Release store per batch.
- **Cached indices.** Keep a producer-local copy of `head` and refresh it only when the queue looks full (and the mirror for the consumer). Benchmark against your first version at cap 1024.
- **MPSC from SPSC rings.** One ring per producer, one consumer polling them round-robin. Benchmark against `std::sync::mpsc` and `crossbeam-channel` with 1, 2 and 4 producers.
