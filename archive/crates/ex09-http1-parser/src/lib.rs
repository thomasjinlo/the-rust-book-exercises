//! Incremental HTTP/1.1 request parser for a server read loop.
//!
//! Do not edit the items marked "provided". Implement `Parser`.
//! `Host` is not validated: a request without it parses.

use std::fmt;

/// Provided. Size limits; `Default` is 8192 / 16384 / 100 / 1 MiB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Request line length in bytes, excluding the CRLF.
    pub max_request_line: usize,
    /// Header section in bytes: every header line plus its CRLF, excluding
    /// the blank line that ends the section. Also bounds a chunked trailer section.
    pub max_header_bytes: usize,
    /// Number of header fields. Also bounds the fields in a chunked trailer section.
    pub max_headers: usize,
    /// Decoded body length in bytes.
    pub max_body: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_request_line: 8192,
            max_header_bytes: 16384,
            max_headers: 100,
            max_body: 1024 * 1024,
        }
    }
}

/// Provided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    Http10,
    Http11,
}

/// Provided. One fully parsed request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// Case preserved.
    pub method: String,
    pub target: String,
    pub version: Version,
    /// In wire order, names case preserved, values with leading and trailing SP/HTAB removed.
    pub headers: Vec<(String, Vec<u8>)>,
    /// Decoded body. Empty when the request has no body headers.
    pub body: Vec<u8>,
}

impl Request {
    /// Provided. First header whose name equals `name` ignoring ASCII case.
    pub fn header(&self, name: &str) -> Option<&[u8]> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_slice())
    }
}

/// Provided. Every way a request can be rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    /// Request line is not `method SP target SP version`, or the method or target is invalid.
    BadRequestLine,
    /// Version is not exactly `HTTP/1.0` or `HTTP/1.1`.
    BadVersion,
    /// A line ended in LF without a preceding CR.
    BareLf,
    /// Header name is empty or not a token, or the line has no colon.
    BadHeaderName,
    /// Header value contains a control byte other than HTAB, or DEL.
    BadHeaderValue,
    /// A header line starts with SP or HTAB.
    ObsFold,
    RequestLineTooLong,
    HeaderTooLarge,
    TooManyHeaders,
    BadContentLength,
    ConflictingContentLength,
    /// Both Content-Length and Transfer-Encoding are present.
    AmbiguousLength,
    UnsupportedTransferEncoding,
    BadChunk,
    BodyTooLarge,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ParseError {}

/// Incremental parser. Feed it whatever the socket returned.
///
/// Add private fields as needed.
pub struct Parser {
    #[allow(dead_code)]
    limits: Limits,
}

impl Parser {
    pub fn new(limits: Limits) -> Parser {
        let _ = limits;
        todo!()
    }

    /// Appends `bytes` and returns every request completed by them, in order.
    /// An incomplete request stays buffered.
    ///
    /// On error the parser is poisoned: this and every later call returns the same
    /// error, requests completed earlier in the same call are discarded, and the
    /// buffer is released (`buffered()` is 0).
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<Request>, ParseError> {
        let _ = bytes;
        todo!()
    }

    /// Bytes held for an incomplete request.
    pub fn buffered(&self) -> usize {
        todo!()
    }
}
