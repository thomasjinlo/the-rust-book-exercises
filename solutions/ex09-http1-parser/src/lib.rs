//! Incremental HTTP/1.1 request parser for a server read loop.
//!
//! Reference solution. Each feed re-parses the buffered request from its first byte.
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
pub struct Parser {
    limits: Limits,
    buf: Vec<u8>,
    error: Option<ParseError>,
}

impl Parser {
    pub fn new(limits: Limits) -> Parser {
        Parser {
            limits,
            buf: Vec::new(),
            error: None,
        }
    }

    /// Appends `bytes` and returns every request completed by them, in order.
    /// An incomplete request stays buffered.
    ///
    /// On error the parser is poisoned: this and every later call returns the same
    /// error, requests completed earlier in the same call are discarded, and the
    /// buffer is released (`buffered()` is 0).
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<Request>, ParseError> {
        if let Some(e) = self.error {
            return Err(e);
        }
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        loop {
            match parse_request(&self.buf, &self.limits) {
                Ok(Some((req, used))) => {
                    self.buf.drain(..used);
                    out.push(req);
                }
                Ok(None) => return Ok(out),
                Err(e) => {
                    self.error = Some(e);
                    self.buf = Vec::new();
                    return Err(e);
                }
            }
        }
    }

    /// Bytes held for an incomplete request.
    pub fn buffered(&self) -> usize {
        self.buf.len()
    }
}

type Headers = Vec<(String, Vec<u8>)>;

/// RFC 9110 tchar.
fn is_tchar(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b)
}

fn is_token(s: &[u8]) -> bool {
    !s.is_empty() && s.iter().all(|&b| is_tchar(b))
}

fn trim_ows(mut s: &[u8]) -> &[u8] {
    while let [b' ' | b'\t', rest @ ..] = s {
        s = rest;
    }
    while let [rest @ .., b' ' | b'\t'] = s {
        s = rest;
    }
    s
}

/// Finds the CRLF-terminated line starting at `start`.
/// Returns `(content_end, next_line_start)`, or `None` if the line is incomplete.
/// A content length above `max` is an error as soon as it is known, even
/// before the CRLF arrives.
fn next_line(
    buf: &[u8],
    start: usize,
    max: usize,
    too_long: ParseError,
) -> Result<Option<(usize, usize)>, ParseError> {
    let rest = &buf[start..];
    match rest.iter().position(|&b| b == b'\n') {
        Some(i) => {
            if i == 0 || rest[i - 1] != b'\r' {
                return Err(ParseError::BareLf);
            }
            if i - 1 > max {
                return Err(too_long);
            }
            Ok(Some((start + i - 1, start + i + 1)))
        }
        None => {
            // A trailing CR may be the first half of the CRLF.
            let pending = rest.strip_suffix(b"\r").unwrap_or(rest);
            if pending.len() > max {
                Err(too_long)
            } else {
                Ok(None)
            }
        }
    }
}

fn parse_header_line(line: &[u8]) -> Result<(String, Vec<u8>), ParseError> {
    if matches!(line[0], b' ' | b'\t') {
        return Err(ParseError::ObsFold);
    }
    let colon = line
        .iter()
        .position(|&b| b == b':')
        .ok_or(ParseError::BadHeaderName)?;
    let name = &line[..colon];
    if !is_token(name) {
        return Err(ParseError::BadHeaderName);
    }
    let value = trim_ows(&line[colon + 1..]);
    if value.iter().any(|&b| (b < 0x20 && b != b'\t') || b == 0x7f) {
        return Err(ParseError::BadHeaderValue);
    }
    Ok((name.iter().map(|&b| b as char).collect(), value.to_vec()))
}

/// Parses header lines up to and including the blank line.
/// Used for the header section and for chunked trailers.
fn parse_header_section(
    buf: &[u8],
    start: usize,
    lim: &Limits,
) -> Result<Option<(Headers, usize)>, ParseError> {
    let mut pos = start;
    let mut used = 0;
    let mut headers = Vec::new();
    loop {
        // Each header line costs its content plus 2 bytes of CRLF.
        let max = (lim.max_header_bytes - used).saturating_sub(2);
        let Some((end, next)) = next_line(buf, pos, max, ParseError::HeaderTooLarge)? else {
            return Ok(None);
        };
        if end == pos {
            return Ok(Some((headers, next)));
        }
        used += next - pos;
        headers.push(parse_header_line(&buf[pos..end])?);
        if headers.len() > lim.max_headers {
            return Err(ParseError::TooManyHeaders);
        }
        pos = next;
    }
}

enum Framing {
    None,
    Length(u64),
    Chunked,
}

fn parse_digits(v: &[u8]) -> Result<u64, ParseError> {
    if v.is_empty() {
        return Err(ParseError::BadContentLength);
    }
    v.iter().try_fold(0u64, |acc, &b| {
        if !b.is_ascii_digit() {
            return Err(ParseError::BadContentLength);
        }
        acc.checked_mul(10)
            .and_then(|a| a.checked_add(u64::from(b - b'0')))
            .ok_or(ParseError::BadContentLength)
    })
}

/// Check order: Content-Length syntax, Content-Length conflict,
/// Content-Length with Transfer-Encoding, Transfer-Encoding value.
fn framing(headers: &Headers) -> Result<Framing, ParseError> {
    let lengths = headers
        .iter()
        .filter(|(n, _)| n.eq_ignore_ascii_case("content-length"))
        .map(|(_, v)| parse_digits(v))
        .collect::<Result<Vec<_>, _>>()?;
    if lengths.windows(2).any(|w| w[0] != w[1]) {
        return Err(ParseError::ConflictingContentLength);
    }
    let mut te = headers
        .iter()
        .filter(|(n, _)| n.eq_ignore_ascii_case("transfer-encoding"))
        .peekable();
    if te.peek().is_none() {
        return Ok(lengths
            .first()
            .map_or(Framing::None, |&n| Framing::Length(n)));
    }
    if !lengths.is_empty() {
        return Err(ParseError::AmbiguousLength);
    }
    let codings: Vec<&[u8]> = te
        .flat_map(|(_, v)| v.split(|&b| b == b','))
        .map(trim_ows)
        .collect();
    // The last coding must be chunked. Earlier codings are not applied:
    // the body keeps whatever encoding they define.
    let (last, earlier) = codings
        .split_last()
        .ok_or(ParseError::UnsupportedTransferEncoding)?;
    let ok = last.eq_ignore_ascii_case(b"chunked")
        && earlier
            .iter()
            .all(|c| is_token(c) && !c.eq_ignore_ascii_case(b"chunked"));
    if ok {
        Ok(Framing::Chunked)
    } else {
        Err(ParseError::UnsupportedTransferEncoding)
    }
}

fn parse_chunk_size(line: &[u8]) -> Result<u64, ParseError> {
    // Everything from the first ';' is a chunk extension and is ignored.
    let digits = line.split(|&b| b == b';').next().unwrap_or(line);
    // 16 hex digits always fit in u64.
    if digits.is_empty() || digits.len() > 16 {
        return Err(ParseError::BadChunk);
    }
    digits.iter().try_fold(0u64, |acc, &b| {
        let d = (b as char).to_digit(16).ok_or(ParseError::BadChunk)?;
        Ok(acc << 4 | u64::from(d))
    })
}

fn parse_chunked(
    buf: &[u8],
    start: usize,
    lim: &Limits,
) -> Result<Option<(Vec<u8>, usize)>, ParseError> {
    let mut pos = start;
    let mut body = Vec::new();
    loop {
        let Some((end, next)) = next_line(buf, pos, lim.max_request_line, ParseError::BadChunk)?
        else {
            return Ok(None);
        };
        let size = parse_chunk_size(&buf[pos..end])?;
        if size == 0 {
            // Trailers are validated like headers, then dropped.
            return Ok(parse_header_section(buf, next, lim)?.map(|(_, end)| (body, end)));
        }
        // body.len() <= max_body always holds, so the subtraction cannot underflow.
        if size > (lim.max_body - body.len()) as u64 {
            return Err(ParseError::BodyTooLarge);
        }
        let data_end = next + size as usize;
        // Check the CRLF after the data byte by byte so a bad one fails early.
        let tail = &buf[data_end.min(buf.len())..];
        if tail.iter().zip(b"\r\n").any(|(a, b)| a != b) {
            return Err(ParseError::BadChunk);
        }
        if tail.len() < 2 {
            return Ok(None);
        }
        body.extend_from_slice(&buf[next..data_end]);
        pos = data_end + 2;
    }
}

/// Parses one request from the start of `buf`.
/// Returns the request and the number of bytes it used, or `None` if more input is needed.
fn parse_request(buf: &[u8], lim: &Limits) -> Result<Option<(Request, usize)>, ParseError> {
    let Some((line_end, next)) =
        next_line(buf, 0, lim.max_request_line, ParseError::RequestLineTooLong)?
    else {
        return Ok(None);
    };
    let mut parts = buf[..line_end].split(|&b| b == b' ');
    let (Some(method), Some(target), Some(version), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(ParseError::BadRequestLine);
    };
    // Target must be printable ASCII so it converts to String losslessly.
    if !is_token(method) || target.is_empty() || !target.iter().all(|b| (0x21..=0x7e).contains(b)) {
        return Err(ParseError::BadRequestLine);
    }
    let version = match version {
        b"HTTP/1.1" => Version::Http11,
        b"HTTP/1.0" => Version::Http10,
        _ => return Err(ParseError::BadVersion),
    };

    let Some((headers, body_start)) = parse_header_section(buf, next, lim)? else {
        return Ok(None);
    };
    let (body, used) = match framing(&headers)? {
        Framing::None => (Vec::new(), body_start),
        Framing::Length(n) => {
            if n > lim.max_body as u64 {
                return Err(ParseError::BodyTooLarge);
            }
            let end = body_start + n as usize;
            if buf.len() < end {
                return Ok(None);
            }
            (buf[body_start..end].to_vec(), end)
        }
        Framing::Chunked => match parse_chunked(buf, body_start, lim)? {
            Some(done) => done,
            None => return Ok(None),
        },
    };
    let req = Request {
        method: method.iter().map(|&b| b as char).collect(),
        target: target.iter().map(|&b| b as char).collect(),
        version,
        headers,
        body,
    };
    Ok(Some((req, used)))
}
