# Exercise 04: Frame codec

**Time:** 2 h | **Crate:** `crates/ex04-frame-codec`

## Task
Turn a byte stream into frames. Each frame is a 4-byte big-endian length `N`, then `N` payload bytes. The stream
arrives in chunks of any size, and a chunk can end in the middle of a frame.

## Example
Two frames, `hi` and an empty one, are these 10 bytes. `N = 0` is a valid empty frame.

```text
byte #:  1  2  3  4 |  5  6 |  7  8  9 10
value : 00 00 00 02 | 68 69 | 00 00 00 00
        \_ N = 2 __/ payload  \_ N = 0 __/
```

`Decoder::new(1024)`, one byte fed at a time, `next_frame()` called after each:

| After byte | `next_frame()` |
|---|---|
| 1 to 5 | `Ok(None)` |
| 6 | `Ok(Some(b"hi".to_vec()))` |
| 7 to 9 | `Ok(None)` |
| 10 | `Ok(Some(vec![]))` |

`Decoder::new(100)`, after feeding `[0, 0, 0, 101]`, `next_frame()` returns `Err(TooLarge)`. It returns the same
error after 500 more bytes are fed.

## API
Provided: `pub struct TooLarge;`, the error type. Create a decoder with `Decoder::new(max_len: u32)`.

### `encode`
```rust
pub fn encode(payload: &[u8], out: &mut Vec<u8>)
```
1. Appends the 4-byte big-endian length of `payload`, then `payload`.
2. Bytes already in `out` are unchanged.

### `feed`
```rust
pub fn feed(&mut self, bytes: &[u8])
```
1. Adds the next chunk of the stream. Any length is allowed, including 0.

### `next_frame`
```rust
pub fn next_frame(&mut self) -> Result<Option<Vec<u8>>, TooLarge>
```
1. If the bytes fed so far hold a complete next frame, returns `Ok(Some(payload))` and consumes it.
2. Otherwise returns `Ok(None)`.
3. As soon as the 4 length bytes show `N > max_len`, returns `Err(TooLarge)`, without waiting for the payload.
   Every later call returns `Err(TooLarge)` too.

## Done when
Edit only `src/lib.rs`. From `crates/ex04-frame-codec`, `cargo test` reports 9 passed (the starter fails all 9).
The hard rule: however a stream is cut into chunks, the decoder returns the same frames. One test checks every
split point of a small stream, one byte at a time, and 200 seeded random chunkings.

## Read first
- [Ch. 3.2: Data Types](https://doc.rust-lang.org/book/ch03-02-data-types.html)
- [Ch. 4.3: The Slice Type](https://doc.rust-lang.org/book/ch04-03-slices.html)
- [Framing (Tokio)](https://tokio.rs/tokio/tutorial/framing)

## Stretch (optional)
- Add a kind byte to each frame.
- Return frames without copying the payload.
