//! Length-prefixed frames on a byte stream.
//!
//! Replace each `todo!()`. Keep the public signatures. Add any private fields and helpers you need.

/// Returned by `Decoder::next_frame` when a length field is greater than the decoder's maximum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TooLarge;

/// Appends one frame to `out`: the payload length as 4 big-endian bytes, then the payload.
pub fn encode(payload: &[u8], out: &mut Vec<u8>) {
    let _ = (payload, out);
    todo!()
}

/// Collects stream bytes and returns the complete frames in them.
pub struct Decoder {
    // Add the fields you need.
}

impl Decoder {
    /// Creates an empty decoder. A length field greater than `max_len` is an error.
    pub fn new(max_len: u32) -> Decoder {
        let _ = max_len;
        todo!()
    }

    /// Adds the next chunk of the stream. Any length is allowed, including 0.
    pub fn feed(&mut self, bytes: &[u8]) {
        let _ = bytes;
        todo!()
    }

    /// Returns the next complete frame's payload, or `Ok(None)` if more bytes are needed.
    pub fn next_frame(&mut self) -> Result<Option<Vec<u8>>, TooLarge> {
        todo!()
    }
}
