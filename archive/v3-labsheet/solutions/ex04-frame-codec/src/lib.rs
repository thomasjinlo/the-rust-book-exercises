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

/// Returned by [`Decoder::next_frame`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// The length field was 0. A frame needs at least the kind byte.
    ZeroLength,
    /// The length field `len` is greater than the decoder's `max`.
    FrameTooLarge { len: u32, max: u32 },
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

impl std::error::Error for DecodeError {}

/// Appends the wire encoding of `frame` to `out`. Bytes already in `out`
/// are left untouched.
///
/// Panics if `1 + payload.len()` does not fit in a `u32`.
pub fn encode(frame: &Frame, out: &mut Vec<u8>) {
    let len = u32::try_from(1 + frame.payload.len()).expect("frame longer than u32::MAX");
    out.extend_from_slice(&len.to_be_bytes());
    out.push(frame.kind);
    out.extend_from_slice(&frame.payload);
}

/// Incremental decoder. Feed it arbitrary chunks, then pull frames.
#[derive(Debug)]
pub struct Decoder {
    buf: Vec<u8>,
    max: u32,
    failed: Option<DecodeError>,
}

impl Decoder {
    /// Creates a decoder that rejects any length field greater than `max_frame_len`.
    pub fn new(max_frame_len: u32) -> Decoder {
        Decoder {
            buf: Vec::new(),
            max: max_frame_len,
            failed: None,
        }
    }

    /// Supplies the next chunk of the byte stream. Never fails.
    pub fn feed(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// Returns the next complete frame, or `Ok(None)` if more bytes are needed.
    /// After an error, every later call returns that same error.
    pub fn next_frame(&mut self) -> Result<Option<Frame>, DecodeError> {
        if let Some(e) = &self.failed {
            return Err(e.clone());
        }
        let Some(header) = self.buf.first_chunk::<4>() else {
            return Ok(None);
        };
        let len = u32::from_be_bytes(*header);
        // Checked on the header alone, before any payload byte arrives.
        let err = if len == 0 {
            Some(DecodeError::ZeroLength)
        } else if len > self.max {
            Some(DecodeError::FrameTooLarge { len, max: self.max })
        } else {
            None
        };
        if let Some(e) = err {
            self.failed = Some(e.clone());
            return Err(e);
        }
        let total = 4 + len as usize;
        if self.buf.len() < total {
            return Ok(None);
        }
        let kind = self.buf[4];
        let payload = self.buf[5..total].to_vec();
        self.buf.drain(..total);
        Ok(Some(Frame { kind, payload }))
    }
}
