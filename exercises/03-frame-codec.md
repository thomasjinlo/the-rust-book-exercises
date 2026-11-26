# Exercise 03: Incremental frame codec for a byte stream

**Time:** ~2.5 h | **Area:** networking / wire protocols | **Crate:** `crates/ex03-frame-codec`
**Builds on:** none (standalone)

## Read first
- Enums and `match`: https://doc.rust-lang.org/book/ch06-00-enums.html
- Vectors and slices: https://doc.rust-lang.org/book/ch08-00-common-collections.html
- Error handling: https://doc.rust-lang.org/book/ch09-00-error-handling.html
- Testing: https://doc.rust-lang.org/book/ch11-00-testing.html
- `u32::from_be_bytes`, `to_be_bytes`: https://doc.rust-lang.org/std/primitive.u32.html
- `Vec::drain`: https://doc.rust-lang.org/std/vec/struct.Vec.html#method.drain
- `std::io::Read` (how streams hand you arbitrary-sized reads): https://doc.rust-lang.org/std/io/trait.Read.html
- TCP is a byte stream with no message boundaries: https://en.wikipedia.org/wiki/Transmission_Control_Protocol#Data_transfer

## What you are building
TCP delivers bytes, not messages. One `read` can return half a message, or three and a half. Every
binary protocol on top of it (database drivers, RPC systems, brokers) therefore marks message
boundaries itself. The common method is a length prefix. You build an encoder and an incremental decoder
for such a protocol. The decoder accepts input in arbitrary chunks, yields complete frames, and
refuses hostile length fields without buffering their payload.

Wire format of one frame:

```text
| L: u32 big-endian | kind: u8 | payload: L-1 bytes |
```

`L` counts the kind byte and the payload, not the 4 length bytes. A frame takes `4 + L` bytes on the wire.

## Starting point
- Edit: `/home/function/.kiro/crew/workspace/rust-exercises/crates/ex03-frame-codec/src/lib.rs`. All bodies are `todo!()`. `Frame`, the error types and their `Display` impls are done; do not change the public signatures.
- Do not edit `tests/codec.rs`. Put your own tests in a new file `tests/extra.rs`.
- Run from the crate directory: `cargo test`.
- Only `proptest` (dev-dependency) is needed. Add no other crates for the main task.

## Requirements
Definitions: `max` is the value passed to `Decoder::new`. "Drained" means `next_frame` was called until it
returned `Ok(None)`.

**Encoding**
- R1. `encode(frame, out)` appends exactly `L` as 4 big-endian bytes, then `kind`, then `payload`, where `L = 1 + payload.len()`. Existing bytes in `out` are untouched.
- R2. `encode_with_limit(frame, max_len, out)` returns `Err(EncodeError::PayloadTooLarge)` when `1 + payload.len() > max_len`. `1 + payload.len() == max_len` succeeds. `encode` is `encode_with_limit` with `max_len = u32::MAX`.
- R3. On `Err`, `out` is unchanged (length and contents). The length check must not overflow when `payload.len()` is `usize::MAX`-sized on a 64-bit target (compute in `u64`).

**Decoding**
- R4. `Decoder::feed` only appends to the internal buffer. It never decodes and never returns an error.
- R5. `next_frame` returns `Ok(Some(frame))` when the buffer starts with a complete frame, and consumes exactly `4 + L` bytes. It returns `Ok(None)` when it needs more bytes, including when fewer than 4 bytes are buffered.
- R6. Frames come out in wire order. Chunk boundaries (any split, including one byte at a time and many frames in one chunk) never change the output.
- R7. A length field of 0 gives `Err(DecodeError::ZeroLength)`.
- R8. A length field `len > max` gives `Err(DecodeError::FrameTooLarge { len, max })`. `len == max` is accepted. The error is returned as soon as 4 length bytes are buffered, with zero payload bytes present.
- R9. Poisoning: after `next_frame` returns an `Err`, every later `next_frame` returns an equal `Err`, and `feed` stores nothing (`buffered_len()` does not change).
- R10. A frame with `L == 1` (empty payload) decodes to `Frame { kind, payload: vec![] }`.

**Memory**
- R11. `buffered_len()` returns `buf.len()` of your internal buffer, counting decoded-but-not-yet-compacted bytes.
- R12. Bound after `feed(chunk)`, given the decoder was drained before the call: `buffered_len() <= 4 + max + chunk.len()`.
- R13. Bound after draining: `buffered_len() < 4 + max`. It equals the size of the one incomplete frame that remains, or 0.
- R14. Feeding 1,000,000 frames of 9 bytes each (4-byte payload) in 4093-byte chunks, draining after each chunk, never leaves `buffered_len() >= 9` after a drain.
- R15. Compaction cost: at most one `memmove` of the remaining partial frame per `next_frame` call that returns `Ok(None)`. Do not shift the buffer once per decoded frame.

## Given tests
All in `tests/codec.rs`. 15 run by default, 1 is `#[ignore]`.

| Test | Checks |
|---|---|
| `encode_exact_bytes` | R1 |
| `round_trip` | R5, R6 |
| `split_at_every_byte_boundary` | R5, R6 (every cut point of a 3-frame stream) |
| `byte_at_a_time` | R5, R6 |
| `many_frames_in_one_feed` | R6 (1000 frames) |
| `zero_length_is_an_error` | R7 |
| `oversize_detected_after_four_bytes` | R8 |
| `partial_length_field_needs_more` | R5 |
| `poisoning` | R9 (oversize error, then valid input) |
| `poisoning_after_good_frames` | R9 (error after two good calls) |
| `empty_payload_frame` | R10 |
| `max_size_boundary` | R2, R3, R8 |
| `buffer_bound_with_large_frames` | R12, R13 |
| `million_small_frames_do_not_grow_the_buffer` | R13, R14 |
| `proptest_random_chunking_round_trip` | R6 (256 seeded cases, random frames and chunking) |
| `throughput_100_mib_of_1_kib_frames` (ignored) | completion only; prints MiB/s |

## Acceptance
1. `cargo test` in the crate directory exits 0 with `15 passed; 0 failed; 1 ignored`.
2. `cargo test --release -- --ignored --nocapture` exits 0. Record the printed `MiB/s` in your notes. No threshold is asserted. For reference, the reference solution printed about 14,500 MiB/s on a dev machine (the data stays in cache, so treat it as an upper bound).
3. `cargo clippy --all-targets -- -D warnings` exits 0.
4. `cargo fmt --check` exits 0.

## Extra tests to write
Put them in `tests/extra.rs`.
1. Header split inside the length field: feed `[0,0]`, drain (`Ok(None)`), feed `[0,5]` plus 5 bytes. Catches reading a short header as 0 or panicking on a slice.
2. `max = u32::MAX`, feed `[0xFF; 4]`. Expect `Ok(None)`, not an error. Catches `len as usize + 4` overflow on 32-bit and `>=` instead of `>`.
3. Oversize header followed by 10 MiB fed in 1 KiB chunks: `buffered_len()` stays 4 after the error. Catches buffering after poisoning.
4. Zero-length frame in the middle of a valid stream: frames before it decode, then `ZeroLength`, then the frames after it never appear.
5. `encode_with_limit` with `max_len = 0`: any frame fails (`1 + 0 > 0`). Catches a `saturating_sub` mistake.
6. Interleave `feed` and `next_frame` randomly (call `next_frame` zero to three times between feeds). Output must still equal the input. Catches state kept only when draining fully.
7. Call `next_frame` 1000 times in a row on an empty decoder: always `Ok(None)`, `buffered_len() == 0`.
8. Feed 64 KiB frames (`max = 65536`) in 1-byte chunks and time it. A decoder that re-parses or compacts per byte becomes visibly quadratic; compare against 4 KiB frames and write down the ratio.

## Stretch (optional)
1. Zero-copy payloads: add the `bytes` crate (pin the version), keep the buffer as `BytesMut`, and return `Bytes` slices via `split_to(...).freeze()`. Count heap allocations per frame with a counting `#[global_allocator]` and compare with the `Vec<u8>` version.
2. Implement the `Decoder` and `Encoder` traits of `tokio_util::codec` for your types and drive them through `FramedRead` over a `tokio::io::duplex` pipe.
3. Add a `read_frame<R: std::io::Read>(r: &mut R, dec: &mut Decoder) -> io::Result<Option<Frame>>` helper that reads 4096 bytes at a time. Handle EOF in the middle of a frame as `io::ErrorKind::UnexpectedEof`.
