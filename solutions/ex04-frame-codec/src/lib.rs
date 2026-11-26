//! Length-prefixed frames on a byte stream.

/// Returned by `Decoder::next_frame` when a length field is greater than the decoder's maximum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TooLarge;

/// Appends one frame to `out`: the payload length as 4 big-endian bytes, then the payload.
pub fn encode(payload: &[u8], out: &mut Vec<u8>) {
    let len = u32::try_from(payload.len()).expect("payload longer than u32::MAX");
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(payload);
}

/// Collects stream bytes and returns the complete frames in them.
pub struct Decoder {
    buf: Vec<u8>,
    max: u32,
    failed: bool,
}

impl Decoder {
    /// Creates an empty decoder. A length field greater than `max_len` is an error.
    pub fn new(max_len: u32) -> Decoder {
        Decoder {
            buf: Vec::new(),
            max: max_len,
            failed: false,
        }
    }

    /// Adds the next chunk of the stream. Any length is allowed, including 0.
    pub fn feed(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// Returns the next complete frame's payload, or `Ok(None)` if more bytes are needed.
    pub fn next_frame(&mut self) -> Result<Option<Vec<u8>>, TooLarge> {
        if self.failed {
            return Err(TooLarge);
        }
        let Some(header) = self.buf.first_chunk::<4>() else {
            return Ok(None);
        };
        let len = u32::from_be_bytes(*header);
        // Decided from the 4 length bytes alone, before any payload arrives.
        if len > self.max {
            self.failed = true;
            return Err(TooLarge);
        }
        let end = 4 + len as usize;
        if self.buf.len() < end {
            return Ok(None);
        }
        let payload = self.buf[4..end].to_vec();
        self.buf.drain(..end);
        Ok(Some(payload))
    }
}
