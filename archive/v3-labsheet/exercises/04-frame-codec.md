# Exercise 04: Frame codec

**Time:** 2 h | **Crate:** `crates/ex04-frame-codec`

## Read first
**Rust Book**
- [Ch. 3.2: Data Types](https://doc.rust-lang.org/book/ch03-02-data-types.html)
- [Ch. 4.3: The Slice Type](https://doc.rust-lang.org/book/ch04-03-slices.html)
- [Ch. 5: Using Structs to Structure Related Data](https://doc.rust-lang.org/book/ch05-00-structs.html)
- [Ch. 6: Enums and Pattern Matching](https://doc.rust-lang.org/book/ch06-00-enums.html)
- [Ch. 8: Common Collections](https://doc.rust-lang.org/book/ch08-00-common-collections.html)
- [Ch. 9: Error Handling](https://doc.rust-lang.org/book/ch09-00-error-handling.html)
- [Ch. 11: Writing Automated Tests](https://doc.rust-lang.org/book/ch11-00-testing.html)

**Standard library**
- [`u32`](https://doc.rust-lang.org/std/primitive.u32.html)

**Background**
- [Framing (Tokio)](https://tokio.rs/tokio/tutorial/framing): turning a byte stream into frames when a read returns a partial frame or several frames.
- [54.1. Overview (PostgreSQL documentation)](https://www.postgresql.org/docs/current/protocol-overview.html): section 54.1.1 describes a production protocol whose messages carry a length field, and why receivers use it to find message boundaries.

## Task
Build an encoder and a decoder for length-prefixed frames on a byte stream. The decoder takes the stream in chunks
of any size and returns each complete frame in order. Decoding a long stream must not use memory in proportion to
its length.

## API
No call may panic, except `encode` as stated below. `max` is the value passed to `Decoder::new`.

### `Frame`
```rust
pub struct Frame { pub kind: u8, pub payload: Vec<u8> }
```
Derives `Debug, Clone, PartialEq, Eq`.

### `DecodeError`
```rust
pub enum DecodeError {
    ZeroLength,
    FrameTooLarge { len: u32, max: u32 },
}
```
Derives `Debug, Clone, PartialEq, Eq`. Implements `Display` and `std::error::Error`.

### `encode`
```rust
pub fn encode(frame: &Frame, out: &mut Vec<u8>)
```
Appends one frame to `out`. Bytes already in `out` are unchanged. A frame is these bytes, in order:

| Bytes | Field | Value |
|---|---|---|
| 4 | `L` | `1 + payload.len()` as a `u32`, most significant byte first |
| 1 | kind | `frame.kind` |
| `L - 1` | payload | `frame.payload` |

`L` counts the kind byte and the payload, not the 4 bytes of `L`. A frame takes `4 + L` bytes.
`encode` panics only if `1 + payload.len()` does not fit in a `u32`.

### `Decoder::new`
```rust
pub fn new(max_frame_len: u32) -> Decoder
```
Creates an empty decoder. A length field greater than `max_frame_len` is an error.

### `Decoder::feed`
```rust
pub fn feed(&mut self, bytes: &[u8])
```
Adds the next chunk of the stream. Any length is allowed, including 0. It never fails.

### `Decoder::next_frame`
```rust
pub fn next_frame(&mut self) -> Result<Option<Frame>, DecodeError>
```
Reads frames in the format under `encode`. Rules:

1. If the bytes fed so far hold a complete next frame, return `Ok(Some(frame))` and consume it.
2. Otherwise return `Ok(None)`. This includes the case of fewer than 4 bytes. Calling again without feeding returns
   `Ok(None)` again.
3. Frames come out in stream order. The frames are the same however the stream is split into chunks.
4. If `L == 1`, the frame has an empty `payload`.
5. Errors are returned as soon as the 4 length bytes have arrived. No payload byte is needed:

   | Length field `L` | Returns |
   |---|---|
   | `0` | `Err(DecodeError::ZeroLength)` |
   | `L > max` | `Err(DecodeError::FrameTooLarge { len: L, max })` |
   | `1 <= L <= max` | rules 1 to 4 |

6. After an error, every later call returns the same error, whatever is fed. No later frame is returned.
7. Memory: feed 1,000,000 frames (kind 1, 4-byte payload, 9,000,000 bytes in total) in 4096-byte chunks. Call
   `next_frame` until `Ok(None)` after each chunk and drop each frame. The live heap grows by at most 65,536 bytes.

## Example
Frame A has kind 1 and payload `hi`, so `L = 3`. Frame B has kind 2 and no payload, so `L = 1`. Together they are
these 12 bytes:

```text
byte #:  1  2  3  4 | 5 | 6  7 | 8  9 10 11 | 12
value : 00 00 00 03 | 01| 68 69| 00 00 00 01 | 02
        \_ L = 3 __/  kind \payload/ \_ L = 1 _/ kind
```

`Decoder::new(1024)`, one byte fed at a time, `next_frame()` called after each:

| After byte | `next_frame()` |
|---|---|
| 1 to 6 | `Ok(None)` |
| 7 | `Ok(Some(Frame { kind: 1, payload: b"hi".to_vec() }))` |
| 8 to 11 | `Ok(None)` |
| 12 | `Ok(Some(Frame { kind: 2, payload: vec![] }))` |

If all 12 bytes are fed at once, three calls return A, B, `Ok(None)`.

`encode(&Frame { kind: 7, payload: b"ab".to_vec() }, &mut vec![0xEE])` leaves `[0xEE, 0, 0, 0, 3, 7, 0x61, 0x62]`.

`Decoder::new(100)`:

| Fed | `next_frame()` |
|---|---|
| `[0, 0, 0, 101]` | `Err(FrameTooLarge { len: 101, max: 100 })` |
| 500 more bytes | the same error |

## Done when
Edit only `src/lib.rs`. Do not edit `tests/codec.rs` or `tests/memory.rs`. Add no dependencies. The starter
compiles, and all 15 of its tests fail until you finish.

From `crates/ex04-frame-codec`, all of these exit 0:
```
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```
`cargo test` reports 14 passed for `tests/codec.rs` and 1 passed for `tests/memory.rs`.

## Extra tests (optional)
Put them in `tests/extra.rs`.
- Feed `[0, 0]`, drain, feed `[0, 5]` and 5 more bytes -> one frame with a 4-byte payload.
- `max = u32::MAX`, feed `[0xFF; 4]` -> `Ok(None)`.
- `max = 100`, feed the length field for 101, then 10,000 payload bytes -> every call returns `FrameTooLarge`.
- A frame, then `[0, 0, 0, 0]`, then a frame -> the first frame, then `ZeroLength` forever.
- A frame with a 65,535-byte payload, `max = 65536`, fed one byte at a time -> decodes correctly.

## Stretch (optional)
- Add `encode_with_limit(frame, max_len, out) -> Result<(), EncodeError>` that returns an error instead of
  panicking and leaves `out` unchanged on failure.
- Add `read_frame<R: std::io::Read>(r: &mut R, dec: &mut Decoder) -> std::io::Result<Option<Frame>>` that reads
  4096 bytes at a time. Report end of input inside a frame as `io::ErrorKind::UnexpectedEof`.
