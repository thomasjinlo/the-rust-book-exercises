//! Length-prefixed frame codec for a byte stream.
//!
//! Wire format of one frame:
//!
//! ```text
//! +----------------+--------+----------------------+
//! | L: u32, big-   | kind   | payload              |
//! | endian         | 1 byte | L - 1 bytes          |
//! +----------------+--------+----------------------+
//! ```
//!
//! `L` counts the kind byte plus the payload, so `L >= 1`. The 4 length
//! bytes are not counted in `L`. A frame occupies `4 + L` bytes on the wire.

use std::fmt;

/// One decoded message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub kind: u8,
    pub payload: Vec<u8>,
}

/// Returned by [`encode`] and [`encode_with_limit`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    /// `1 + payload.len()` exceeds the limit (`u32::MAX` for [`encode`]).
    PayloadTooLarge,
}

/// Returned by [`Decoder::next_frame`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// The length field was 0. A frame needs at least the kind byte.
    ZeroLength,
    /// The length field `len` is greater than the decoder's `max`.
    FrameTooLarge { len: u32, max: u32 },
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EncodeError::PayloadTooLarge => write!(f, "payload too large"),
        }
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::ZeroLength => write!(f, "frame length is zero"),
            DecodeError::FrameTooLarge { len, max } => {
                write!(f, "frame length {len} exceeds maximum {max}")
            }
        }
    }
}

impl std::error::Error for EncodeError {}
impl std::error::Error for DecodeError {}

/// Appends the wire encoding of `frame` to `out`, with limit `u32::MAX`.
///
/// On error `out` is left unchanged.
pub fn encode(frame: &Frame, out: &mut Vec<u8>) -> Result<(), EncodeError> {
    let _ = (frame, out);
    todo!("R1: delegate to encode_with_limit")
}

/// Appends the wire encoding of `frame` to `out`.
///
/// Fails with [`EncodeError::PayloadTooLarge`] when `1 + payload.len()`
/// is greater than `max_len` or greater than `u32::MAX`. A frame with
/// `1 + payload.len() == max_len` is accepted. On error `out` is left unchanged.
pub fn encode_with_limit(
    frame: &Frame,
    max_len: u32,
    out: &mut Vec<u8>,
) -> Result<(), EncodeError> {
    let _ = (frame, max_len, out);
    todo!("R1, R2")
}

/// Incremental decoder. Feed it arbitrary chunks, then pull frames.
#[derive(Debug)]
pub struct Decoder {
    // Add the fields you need: the byte buffer, a read offset, the
    // maximum frame length and the poison state.
}

impl Decoder {
    /// Creates a decoder that rejects any length field greater than `max_frame_len`.
    pub fn new(max_frame_len: u32) -> Decoder {
        let _ = max_frame_len;
        todo!()
    }

    /// Appends `bytes` to the internal buffer. Never decodes and never fails.
    /// Ignores the input once the decoder is poisoned.
    pub fn feed(&mut self, bytes: &[u8]) {
        let _ = bytes;
        todo!()
    }

    /// Returns the next complete frame, `Ok(None)` if more bytes are needed,
    /// or an error. After an error the decoder is poisoned: every later call
    /// returns a clone of the same error.
    pub fn next_frame(&mut self) -> Result<Option<Frame>, DecodeError> {
        todo!()
    }

    /// Number of bytes currently held in the internal buffer, including any
    /// already-decoded bytes that have not been compacted away yet.
    pub fn buffered_len(&self) -> usize {
        todo!()
    }
}
