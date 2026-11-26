# Exercise 04: Incremental frame codec for a byte stream

**Time:** ~2 h | **Area:** networking / wire protocols | **Crate:** `crates/ex04-frame-codec`
**Builds on:** none (standalone)

## Read first
- Enums and `match`: https://doc.rust-lang.org/book/ch06-00-enums.html
- Vectors: https://doc.rust-lang.org/book/ch08-01-vectors.html
- Slices: https://doc.rust-lang.org/book/ch04-03-slices.html
- Error handling: https://doc.rust-lang.org/book/ch09-00-error-handling.html
- Testing: https://doc.rust-lang.org/book/ch11-00-testing.html
- `u32::from_be_bytes` and `to_be_bytes`: https://doc.rust-lang.org/std/primitive.u32.html
- `Vec::drain`: https://doc.rust-lang.org/std/vec/struct.Vec.html#method.drain

## What you are building
TCP delivers bytes, not messages. One `read` can return half a message, or three and a half. Binary protocols on top of TCP (database drivers, RPC systems, brokers) therefore mark message boundaries themselves. The common method is a length prefix. You build an encoder and an incremental decoder for such a protocol. The decoder accepts input in arbitrary chunks, returns complete frames, and rejects a hostile length field as soon as it has read it.

## Concepts
A frame on the wire:

```text
| L: u32 big-endian | kind: u8 | payload: L-1 bytes |
```

`L` counts the kind byte and the payload, not the 4 length bytes. A frame takes `4 + L` bytes. Big-endian means the most significant byte comes first: `L = 3` is `00 00 00 03`.

The decoder keeps a buffer of bytes fed so far. `next_frame` looks at the front of the buffer:

1. Fewer than 4 bytes: the length is unknown. Return `Ok(None)`.
2. Read `L`. If `L` is 0 or above the maximum, return an error. Do this before any payload arrives, so a sender cannot make you buffer 4 GiB.
3. Fewer than `4 + L` bytes: return `Ok(None)`.
4. Otherwise remove the first `4 + L` bytes from the buffer and return them as a `Frame`.

Worked example. Frame A has kind 1 and payload `hi`, so `L = 3`. Frame B has kind 2 and no payload, so `L = 1`. The wire holds 12 bytes:

```text
byte #:  1  2  3  4 | 5 | 6  7 | 8  9 10 11 | 12
value : 00 00 00 03 | 01| 68 69| 00 00 00 01 | 02
        \_ L = 3 __/  kind \payload/ \_ L = 1 _/ kind
        \________ frame A ________/ \_____ frame B ____/
```

Feed one byte, then call `next_frame` once:

| After byte | `next_frame()` | `buffered_len()` | Why |
|---|---|---|---|
| 1 to 6 | `Ok(None)` | 1 to 6 | Header incomplete (1 to 3), then payload incomplete (4 to 6). |
| 7 | `Ok(Some(A))` | 0 | 7 bytes are `4 + 3`. The frame is removed. |
| 8 to 10 | `Ok(None)` | 1 to 3 | Header of B incomplete. |
| 11 | `Ok(None)` | 4 | `L = 1`, so 5 bytes are needed. Only 4 are here. |
| 12 | `Ok(Some(B))` | 0 | 5 bytes present. |

If all 12 bytes arrive in one `feed`, the first `next_frame` returns A, the second returns B, the third returns `Ok(None)`. The caller loops until `Ok(None)`.

## Starting point
- Edit: `/home/function/.kiro/crew/workspace/rust-exercises/crates/ex04-frame-codec/src/lib.rs`. Bodies are `todo!()`. `Frame`, `DecodeError` and its `Display` impl are done. Do not change public signatures.
- Do not edit `tests/codec.rs`. Put your own tests in a new file `tests/extra.rs`.
- Run from the crate directory: `cargo test`. The crate has no dependencies. Add none.

## Milestones
Run the given filter after each stage. Stop and fix before moving on.

1. **Encoder.** R1. Run `cargo test encode`. Green: `encode_exact_bytes`, `encode_empty_payload` (2 tests).
2. **Decode whole frames.** R2, R3, R9, R10. Add the buffer and `feed`, then `next_frame` for input that already holds complete frames. Use `u32::from_be_bytes` and `Vec::drain`. Run `cargo test -- --exact round_trip empty_payload_frame`. Green: `round_trip`, `empty_payload_frame` (2 tests).
3. **Partial input.** R4, R5. Return `Ok(None)` for short header and short payload. Run `cargo test partial`, `cargo test trickle`, `cargo test byte_at_a_time`, `cargo test split_at`, `cargo test random_chunking`. Green: `partial_input_needs_more`, `trickle_two_frames`, `byte_at_a_time`, `split_at_every_byte_boundary`, `random_chunking_round_trip` (5 tests).
4. **Errors.** R6, R7, R8. Run `cargo test length` and `cargo test error` and `cargo test oversize`. Green: `zero_length_is_an_error`, `oversize_detected_after_four_bytes`, `length_equal_to_max_is_accepted`, `error_is_returned_again` (4 tests).

## Requirements
`max` is the value passed to `Decoder::new`. "Drained" means `next_frame` was called until it returned `Ok(None)`.

- R1. `encode(frame, out)` appends 4 bytes of `L` in big-endian order, then `kind`, then `payload`, where `L = 1 + payload.len()`. Bytes already in `out` are untouched. It panics if `L` does not fit in a `u32`.
- R2. `Decoder::feed` only appends to the internal buffer. It never decodes and never fails.
- R3. `next_frame` returns `Ok(Some(frame))` when the buffer starts with a complete frame. It removes exactly `4 + L` bytes from the buffer.
- R4. `next_frame` returns `Ok(None)` when the buffer starts with an incomplete frame, including when fewer than 4 bytes are buffered. It does not remove bytes in that case.
- R5. Frames come out in wire order. Where the chunk boundaries fall (byte at a time, every split point, many frames in one chunk) never changes the frames returned.
- R6. A length field of 0 gives `Err(DecodeError::ZeroLength)`.
- R7. A length field `len` greater than `max` gives `Err(DecodeError::FrameTooLarge { len, max })`. `len == max` is accepted. The error is returned once 4 length bytes are buffered, even if no payload byte has arrived.
- R8. After `next_frame` returns an `Err`, every later `next_frame` call returns that same error, whatever is fed afterwards.
- R9. A frame with `L == 1` decodes to `Frame { kind, payload: vec![] }`.
- R10. `buffered_len()` is the number of fed bytes not yet returned as part of a frame. It is 0 after a drain when the fed bytes ended on a frame boundary.

## Common mistakes
- Treating `L` as the payload length. It also counts the kind byte, so the payload has `L - 1` bytes and the frame has `4 + L` bytes.
- Indexing `buf[..4]` before checking `buf.len() >= 4`. This panics on a short header. `first_chunk::<4>()` or `buf.get(..4)` returns `None` instead.
- Waiting for the whole payload before checking `L` against `max`. The oversize test feeds 4 bytes only.
- Computing `4 + len as usize` before comparing `len` to `max`. Compare first, in `u32`.
- Removing bytes from the buffer on the `Ok(None)` path. The next `feed` then continues from a corrupted stream.
- Returning at most one frame per `feed` and leaving the rest unread. Several frames can sit in the buffer, so each `next_frame` call must work from the front of whatever is left.

## Given tests
All in `tests/codec.rs`: 13 tests, none ignored.

| Test | Requirement it checks |
|---|---|
| `encode_exact_bytes` | R1 |
| `encode_empty_payload` | R1 |
| `round_trip` | R3, R5, R10 |
| `empty_payload_frame` | R9 |
| `partial_input_needs_more` | R4, R10 |
| `trickle_two_frames` | R3, R4, R10 (the table in Concepts) |
| `byte_at_a_time` | R5 |
| `split_at_every_byte_boundary` | R5 (every cut point of a 3-frame stream) |
| `random_chunking_round_trip` | R5 (200 cases, seeded PRNG in the test) |
| `zero_length_is_an_error` | R6 |
| `oversize_detected_after_four_bytes` | R7 |
| `length_equal_to_max_is_accepted` | R7 |
| `error_is_returned_again` | R8 |

## Acceptance
1. `cargo test` in the crate directory exits 0 and reports `13 passed; 0 failed` for `tests/codec.rs`.
2. `cargo clippy --all-targets -- -D warnings` exits 0.
3. `cargo fmt --check` exits 0.

## Extra tests to write
Put them in `tests/extra.rs`.
1. Feed `[0, 0]`, drain, then feed `[0, 5]` and 5 more bytes. Expect one frame with a 4-byte payload. Catches a short header read as 0 or a slice panic.
2. `max = u32::MAX`, feed `[0xFF; 4]`. Expect `Ok(None)`, not an error. Catches `>=` instead of `>`.
3. `max = 100`, feed the header for `L = 101`, then 10,000 payload bytes. The first `next_frame` is `FrameTooLarge`. Every later call returns the same error.
4. A zero-length header between two valid frames: the first frame decodes, then `ZeroLength` forever. The frame after it never appears.
5. Call `next_frame` 1000 times in a row on an empty decoder. Always `Ok(None)`, `buffered_len() == 0`.
6. Call `next_frame` zero to three times between random `feed` calls. Output must still equal the input. Catches state that only works when draining fully.
7. A frame with a 65,535-byte payload and `max = 65536`, fed in 1-byte chunks. It decodes correctly. Catches `u16` casts.

## Stretch (optional)
1. Add `encode_with_limit(frame, max_len, out) -> Result<(), EncodeError>` that returns an error instead of panicking and leaves `out` unchanged on failure.
2. Replace `Vec::drain` with a read offset into the buffer, compacting only when `next_frame` returns `Ok(None)`. Write a test that feeds 1,000,000 small frames and checks `buffered_len()` stays small.
3. Add `read_frame<R: std::io::Read>(r: &mut R, dec: &mut Decoder) -> std::io::Result<Option<Frame>>` that reads 4096 bytes at a time. Report EOF in the middle of a frame as `io::ErrorKind::UnexpectedEof`.
