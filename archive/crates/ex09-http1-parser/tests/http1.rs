use ex09_http1_parser::*;
use proptest::prelude::*;

fn parse_all(input: &[u8]) -> Result<Vec<Request>, ParseError> {
    Parser::new(Limits::default()).feed(input)
}

fn parse_with(limits: Limits, input: &[u8]) -> Result<Vec<Request>, ParseError> {
    Parser::new(limits).feed(input)
}

fn parse_one(input: &[u8]) -> Request {
    let mut reqs = parse_all(input).expect("input should parse");
    assert_eq!(reqs.len(), 1, "expected exactly one request");
    reqs.remove(0)
}

fn err_of(input: &[u8]) -> ParseError {
    parse_all(input).expect_err("input should be rejected")
}

/// Feeds one byte at a time and returns the first error or all requests.
fn parse_bytewise(limits: Limits, input: &[u8]) -> Result<Vec<Request>, ParseError> {
    let mut p = Parser::new(limits);
    let mut out = Vec::new();
    for b in input {
        out.extend(p.feed(std::slice::from_ref(b))?);
    }
    Ok(out)
}

fn small_limits() -> Limits {
    Limits {
        max_request_line: 16,
        max_header_bytes: 32,
        max_headers: 3,
        max_body: 8,
    }
}

// R1
#[test]
fn r01_method_tokens_keep_case() {
    for m in ["GET", "get", "M-SEARCH", "X.Y_z~1", "PATCH"] {
        let req = parse_one(format!("{m} / HTTP/1.1\r\n\r\n").as_bytes());
        assert_eq!(req.method, m);
    }
}

#[test]
fn r01_bad_method() {
    for line in [
        "G(T / HTTP/1.1",
        "G@T / HTTP/1.1",
        " / HTTP/1.1",
        "GET /",
        "GET / HTTP/1.1 ",
    ] {
        let input = format!("{line}\r\n\r\n");
        assert_eq!(
            err_of(input.as_bytes()),
            ParseError::BadRequestLine,
            "{line}"
        );
    }
}

// R2
#[test]
fn r02_target_accepted() {
    let req = parse_one(b"GET /a/b?x=1&y=%20#f HTTP/1.1\r\n\r\n");
    assert_eq!(req.target, "/a/b?x=1&y=%20#f");
    assert_eq!(parse_one(b"OPTIONS * HTTP/1.1\r\n\r\n").target, "*");
}

#[test]
fn r02_target_rejected() {
    let cases: [&[u8]; 4] = [
        b"GET  HTTP/1.1\r\n\r\n",
        b"GET /a\x01b HTTP/1.1\r\n\r\n",
        b"GET /a\tb HTTP/1.1\r\n\r\n",
        b"GET /\xc3\xa9 HTTP/1.1\r\n\r\n",
    ];
    for c in cases {
        assert_eq!(err_of(c), ParseError::BadRequestLine, "{c:?}");
    }
    assert_eq!(
        err_of(b"GET /a /b HTTP/1.1\r\n\r\n"),
        ParseError::BadRequestLine
    );
}

// R3
#[test]
fn r03_versions() {
    assert_eq!(
        parse_one(b"GET / HTTP/1.0\r\n\r\n").version,
        Version::Http10
    );
    assert_eq!(
        parse_one(b"GET / HTTP/1.1\r\n\r\n").version,
        Version::Http11
    );
    for v in [
        "HTTP/1.2",
        "HTTP/2.0",
        "HTTP/0.9",
        "http/1.1",
        "HTTP/1.10",
        "HTTP/1.",
        "",
    ] {
        let input = format!("GET / {v}\r\n\r\n");
        assert_eq!(err_of(input.as_bytes()), ParseError::BadVersion, "{v:?}");
    }
}

// R4
#[test]
fn r04_bare_lf() {
    let cases: [&[u8]; 5] = [
        b"GET / HTTP/1.1\nHost: a\r\n\r\n",
        b"GET / HTTP/1.1\r\nHost: a\n\r\n",
        b"GET / HTTP/1.1\r\nHost: a\r\n\n",
        b"GET / HTTP/1.1\r\n\n",
        b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n3\nabc\r\n0\r\n\r\n",
    ];
    for c in cases {
        assert_eq!(
            err_of(c),
            ParseError::BareLf,
            "{:?}",
            String::from_utf8_lossy(c)
        );
    }
}

// R5
#[test]
fn r05_header_names() {
    let req = parse_one(b"GET / HTTP/1.1\r\nX-Custom_1.a: v\r\nhost: h\r\n\r\n");
    assert_eq!(req.headers[0].0, "X-Custom_1.a");
    assert_eq!(req.header("HOST"), Some(&b"h"[..]));
    let cases: [&[u8]; 5] = [
        b"GET / HTTP/1.1\r\nHost : a\r\n\r\n",
        b"GET / HTTP/1.1\r\n: a\r\n\r\n",
        b"GET / HTTP/1.1\r\nHo st: a\r\n\r\n",
        b"GET / HTTP/1.1\r\nHost\r\n\r\n",
        b"GET / HTTP/1.1\r\nH(st: a\r\n\r\n",
    ];
    for c in cases {
        assert_eq!(
            err_of(c),
            ParseError::BadHeaderName,
            "{:?}",
            String::from_utf8_lossy(c)
        );
    }
}

#[test]
fn r05_header_lookup_is_first_match_case_insensitive() {
    let req = parse_one(b"GET / HTTP/1.1\r\nX-A: 1\r\nx-a: 2\r\n\r\n");
    assert_eq!(req.header("X-A"), Some(&b"1"[..]));
    assert_eq!(req.header("missing"), None);
    assert_eq!(req.headers.len(), 2);
}

// R6
#[test]
fn r06_values_trim_ows() {
    let req = parse_one(b"GET / HTTP/1.1\r\nA:   x y \t\r\nB:\r\nC:\t\t\r\nD:z\r\n\r\n");
    assert_eq!(req.header("A"), Some(&b"x y"[..]));
    assert_eq!(req.header("B"), Some(&b""[..]));
    assert_eq!(req.header("C"), Some(&b""[..]));
    assert_eq!(req.header("D"), Some(&b"z"[..]));
}

#[test]
fn r06_value_control_bytes() {
    for bad in [
        &b"a\x00b"[..],
        b"a\x0bb",
        b"a\rb",
        b"a\x7fb",
        b"\x0bchunked",
    ] {
        let mut input = b"GET / HTTP/1.1\r\nA: ".to_vec();
        input.extend_from_slice(bad);
        input.extend_from_slice(b"\r\n\r\n");
        assert_eq!(err_of(&input), ParseError::BadHeaderValue, "{bad:?}");
    }
    let req = parse_one(b"GET / HTTP/1.1\r\nA: a\tb\xff\r\n\r\n");
    assert_eq!(req.header("A"), Some(&b"a\tb\xff"[..]));
}

// R7
#[test]
fn r07_obs_fold() {
    let cases: [&[u8]; 3] = [
        b"GET / HTTP/1.1\r\nA: b\r\n c\r\n\r\n",
        b"GET / HTTP/1.1\r\nA: b\r\n\tc\r\n\r\n",
        b"GET / HTTP/1.1\r\n A: b\r\n\r\n",
    ];
    for c in cases {
        assert_eq!(
            err_of(c),
            ParseError::ObsFold,
            "{:?}",
            String::from_utf8_lossy(c)
        );
    }
}

// R8
#[test]
fn r08_request_line_limit() {
    let l = small_limits();
    // Limit is 16 bytes of content, CRLF excluded.
    let at_limit = format!(
        "GET /{} HTTP/1.1\r\n\r\n",
        "a".repeat(16 - "GET / HTTP/1.1".len())
    );
    assert!(parse_with(l, at_limit.as_bytes()).is_ok());
    let over = format!(
        "GET /{} HTTP/1.1\r\n\r\n",
        "a".repeat(16 - "GET / HTTP/1.1".len() + 1)
    );
    assert_eq!(
        parse_with(l, over.as_bytes()),
        Err(ParseError::RequestLineTooLong)
    );
}

#[test]
fn r08_request_line_limit_detected_without_crlf() {
    let l = small_limits();
    assert_eq!(
        parse_with(l, &[b'a'; 17]),
        Err(ParseError::RequestLineTooLong)
    );
    // 16 bytes plus a CR that may start the CRLF is still incomplete, not an error.
    let mut p = Parser::new(l);
    let mut input = vec![b'a'; 16];
    input.push(b'\r');
    assert_eq!(p.feed(&input), Ok(vec![]));
}

#[test]
fn r08_header_bytes_limit() {
    let l = Limits {
        max_header_bytes: 20,
        ..Limits::default()
    };
    // Header lines count with their CRLF: "A: 123456789012\r\n" is 17 bytes, "B: 12\r\n" is 7.
    let over = b"GET / HTTP/1.1\r\nA: 123456789012\r\nB: 12\r\n\r\n";
    assert_eq!(parse_with(l, over), Err(ParseError::HeaderTooLarge));
    // The blank line does not count: 20 bytes pass, 21 fail.
    let exactly = b"GET / HTTP/1.1\r\nA: 123456789012345\r\n\r\n";
    assert!(parse_with(l, exactly).is_ok());
    let one_more = b"GET / HTTP/1.1\r\nA: 1234567890123456\r\n\r\n";
    assert_eq!(parse_with(l, one_more), Err(ParseError::HeaderTooLarge));
}

#[test]
fn r08_header_bytes_detected_without_terminator() {
    let l = Limits {
        max_header_bytes: 20,
        ..Limits::default()
    };
    let mut input = b"GET / HTTP/1.1\r\nA: ".to_vec();
    input.extend_from_slice(&[b'x'; 40]);
    assert_eq!(parse_with(l, &input), Err(ParseError::HeaderTooLarge));
}

#[test]
fn r08_too_many_headers() {
    let l = Limits {
        max_headers: 2,
        ..Limits::default()
    };
    assert!(parse_with(l, b"GET / HTTP/1.1\r\nA: 1\r\nB: 2\r\n\r\n").is_ok());
    // No blank line yet: the third field is enough.
    assert_eq!(
        parse_with(l, b"GET / HTTP/1.1\r\nA: 1\r\nB: 2\r\nC: 3\r\n"),
        Err(ParseError::TooManyHeaders)
    );
}

// R9
#[test]
fn r09_content_length_body() {
    let req = parse_one(b"POST / HTTP/1.1\r\nContent-Length: 5\r\n\r\nhello");
    assert_eq!(req.body, b"hello");
    let req = parse_one(b"POST / HTTP/1.1\r\ncontent-length:  0 \r\n\r\n");
    assert!(req.body.is_empty());
    let req = parse_one(b"POST / HTTP/1.1\r\nContent-Length: 007\r\n\r\nabcdefg");
    assert_eq!(req.body, b"abcdefg");
}

#[test]
fn r09_bad_content_length() {
    let values = [
        "",
        "+5",
        "-5",
        "5 5",
        "0x5",
        "5,5",
        "5.0",
        "five",
        "18446744073709551616",
        "99999999999999999999999999",
    ];
    for v in values {
        let input = format!("POST / HTTP/1.1\r\nContent-Length: {v}\r\n\r\n");
        assert_eq!(
            err_of(input.as_bytes()),
            ParseError::BadContentLength,
            "{v:?}"
        );
    }
}

#[test]
fn r09_waits_for_full_body() {
    let mut p = Parser::new(Limits::default());
    assert_eq!(
        p.feed(b"POST / HTTP/1.1\r\nContent-Length: 5\r\n\r\nhel"),
        Ok(vec![])
    );
    let reqs = p.feed(b"lo").unwrap();
    assert_eq!(reqs[0].body, b"hello");
}

// R10
#[test]
fn r10_conflicting_content_length() {
    let input = b"POST / HTTP/1.1\r\nContent-Length: 5\r\nContent-Length: 6\r\n\r\nhello!";
    assert_eq!(err_of(input), ParseError::ConflictingContentLength);
}

#[test]
fn r10_identical_content_length_accepted() {
    let req = parse_one(b"POST / HTTP/1.1\r\nContent-Length: 5\r\ncontent-length: 5\r\n\r\nhello");
    assert_eq!(req.body, b"hello");
    assert_eq!(req.headers.len(), 2);
}

// R11
#[test]
fn r11_ambiguous_length() {
    let a = b"POST / HTTP/1.1\r\nContent-Length: 4\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n";
    let b = b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\nContent-Length: 4\r\n\r\n0\r\n\r\n";
    assert_eq!(err_of(a), ParseError::AmbiguousLength);
    assert_eq!(err_of(b), ParseError::AmbiguousLength);
}

// R12
#[test]
fn r12_chunked_basic() {
    let req = parse_one(
        b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n",
    );
    assert_eq!(req.body, b"hello world");
    assert_eq!(req.header("transfer-encoding"), Some(&b"chunked"[..]));
}

#[test]
fn r12_chunked_is_case_insensitive_and_hex() {
    let req = parse_one(
        b"POST / HTTP/1.1\r\nTransfer-Encoding:  ChUnKeD \r\n\r\nA\r\n0123456789\r\n0a\r\nabcdefghij\r\n0\r\n\r\n",
    );
    assert_eq!(req.body, b"0123456789abcdefghij");
}

#[test]
fn r12_chunk_extensions_ignored() {
    let req = parse_one(
        b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n3;name=val;x\r\nabc\r\n0;last\r\n\r\n",
    );
    assert_eq!(req.body, b"abc");
}

#[test]
fn r12_trailers_discarded() {
    let mut p = Parser::new(Limits::default());
    let input = b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n1\r\na\r\n0\r\nX-Sum: 1\r\nY: 2\r\n\r\nGET /next HTTP/1.1\r\n\r\n";
    let reqs = p.feed(input).unwrap();
    assert_eq!(reqs.len(), 2);
    assert_eq!(reqs[0].body, b"a");
    assert_eq!(reqs[0].headers.len(), 1);
    assert_eq!(reqs[0].header("X-Sum"), None);
    assert_eq!(reqs[1].target, "/next");
}

#[test]
fn r12_trailers_are_validated() {
    let bad_name = b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n0\r\nX : 1\r\n\r\n";
    assert_eq!(err_of(bad_name), ParseError::BadHeaderName);
    let l = Limits {
        max_headers: 1,
        ..Limits::default()
    };
    let many = b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n0\r\nA: 1\r\nB: 2\r\n";
    assert_eq!(parse_with(l, many), Err(ParseError::TooManyHeaders));
}

// R13
#[test]
fn r13_unsupported_transfer_encoding() {
    for v in [
        "gzip",
        "identity",
        "chunked, gzip",
        "xchunked",
        "chunkedx",
        "",
        "chunked, chunked",
        ",chunked",
        "chunked,",
    ] {
        let input = format!("POST / HTTP/1.1\r\nTransfer-Encoding: {v}\r\n\r\n0\r\n\r\n");
        assert_eq!(
            err_of(input.as_bytes()),
            ParseError::UnsupportedTransferEncoding,
            "{v:?}"
        );
    }
}

#[test]
fn r13_chunked_last_after_other_coding_is_decoded_not_unwrapped() {
    let req = parse_one(
        b"POST / HTTP/1.1\r\nTransfer-Encoding: gzip, chunked\r\n\r\n2\r\n\x1f\x8b\r\n0\r\n\r\n",
    );
    assert_eq!(req.body, b"\x1f\x8b");
}

#[test]
fn r13_split_transfer_encoding_headers() {
    let req = parse_one(
        b"POST / HTTP/1.1\r\nTransfer-Encoding: gzip\r\nTransfer-Encoding: chunked\r\n\r\n1\r\nz\r\n0\r\n\r\n",
    );
    assert_eq!(req.body, b"z");
}

// R14
#[test]
fn r14_bad_chunk_sizes() {
    let sizes = [
        "",
        "g",
        "-1",
        "+1",
        "0x5",
        " 5",
        "5 ",
        "10000000000000000",
        "fffffffffffffffff",
    ];
    for s in sizes {
        let input =
            format!("POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n{s}\r\nabc\r\n0\r\n\r\n");
        assert_eq!(err_of(input.as_bytes()), ParseError::BadChunk, "{s:?}");
    }
}

#[test]
fn r14_sixteen_digits_parse_then_hit_body_limit() {
    let input = b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\nffffffffffffffff\r\n";
    assert_eq!(err_of(input), ParseError::BodyTooLarge);
    let zeros = b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n0000000000000003\r\nabc\r\n0\r\n\r\n";
    assert_eq!(parse_one(zeros).body, b"abc");
}

#[test]
fn r14_bad_chunk_terminator() {
    let cases: [&[u8]; 3] = [
        b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabcXY0\r\n\r\n",
        b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabcd\r\n0\r\n\r\n",
        b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nab\r\n\r\n0\r\n\r\n",
    ];
    for c in cases {
        assert_eq!(err_of(c), ParseError::BadChunk);
    }
}

#[test]
fn r14_chunk_size_line_limit() {
    let l = small_limits();
    let ext = "x".repeat(40);
    let input = format!("POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n1;{ext}");
    assert_eq!(parse_with(l, input.as_bytes()), Err(ParseError::BadChunk));
}

// R15
#[test]
fn r15_content_length_over_limit_is_immediate() {
    let l = small_limits();
    let input = b"POST / HTTP/1.1\r\nContent-Length: 9\r\n\r\n";
    assert_eq!(parse_with(l, input), Err(ParseError::BodyTooLarge));
    let ok = parse_with(l, b"POST / HTTP/1.1\r\nContent-Length: 8\r\n\r\n12345678").unwrap();
    assert_eq!(ok[0].body.len(), 8);
}

#[test]
fn r15_chunked_total_over_limit() {
    let l = small_limits();
    // 5 + 4 = 9 > 8, detected from the second size line without its data.
    let input = b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n4\r\n";
    assert_eq!(parse_with(l, input), Err(ParseError::BodyTooLarge));
    let ok =
        b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n3\r\nabc\r\n0\r\n\r\n";
    assert_eq!(parse_with(l, ok).unwrap()[0].body.len(), 8);
}

// R16
#[test]
fn r16_no_body_headers_means_empty_body() {
    let mut p = Parser::new(Limits::default());
    let reqs = p
        .feed(b"POST / HTTP/1.1\r\n\r\nGET /x HTTP/1.1\r\n\r\n")
        .unwrap();
    assert_eq!(reqs.len(), 2);
    assert!(reqs[0].body.is_empty());
    assert_eq!(reqs[1].target, "/x");
}

// R17
#[test]
fn r17_http10_and_http11_parse_alike() {
    let bodies = [
        "Content-Length: 3\r\n\r\nabc",
        "Transfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n0\r\n\r\n",
    ];
    for b in bodies {
        let a = parse_one(format!("POST /p HTTP/1.0\r\nHost: h\r\n{b}").as_bytes());
        let c = parse_one(format!("POST /p HTTP/1.1\r\nHost: h\r\n{b}").as_bytes());
        assert_eq!(a.version, Version::Http10);
        assert_eq!(
            (a.method, a.target, a.headers, a.body),
            (c.method, c.target, c.headers, c.body)
        );
    }
}

// R18
#[test]
fn r18_host_is_not_validated() {
    assert_eq!(parse_one(b"GET / HTTP/1.1\r\n\r\n").header("host"), None);
    let req = parse_one(b"GET / HTTP/1.1\r\nHost: not a host!!\r\nHost: second\r\n\r\n");
    assert_eq!(req.header("host"), Some(&b"not a host!!"[..]));
}

// R19
#[test]
fn r19_error_poisons_parser() {
    let mut p = Parser::new(Limits::default());
    assert_eq!(
        p.feed(b"GET / HTTP/9.9\r\n\r\n"),
        Err(ParseError::BadVersion)
    );
    assert_eq!(p.buffered(), 0);
    assert_eq!(
        p.feed(b"GET / HTTP/1.1\r\n\r\n"),
        Err(ParseError::BadVersion)
    );
    assert_eq!(p.feed(b""), Err(ParseError::BadVersion));
}

#[test]
fn r19_error_discards_requests_from_same_feed() {
    let mut p = Parser::new(Limits::default());
    let r = p.feed(b"GET /ok HTTP/1.1\r\n\r\nGET /bad HTTP/1.1\nX\r\n\r\n");
    assert_eq!(r, Err(ParseError::BareLf));
}

// R20
#[test]
fn r20_buffered_tracks_partial_input() {
    let mut p = Parser::new(Limits::default());
    assert_eq!(p.buffered(), 0);
    let full = b"POST / HTTP/1.1\r\nContent-Length: 3\r\n\r\nabc";
    for (i, b) in full.iter().enumerate() {
        let out = p.feed(std::slice::from_ref(b)).unwrap();
        if i + 1 < full.len() {
            assert!(out.is_empty());
            assert_eq!(p.buffered(), i + 1);
        } else {
            assert_eq!(out.len(), 1);
            assert_eq!(p.buffered(), 0);
        }
    }
}

#[test]
fn r20_buffered_keeps_start_of_next_request() {
    let mut p = Parser::new(Limits::default());
    let reqs = p.feed(b"GET /a HTTP/1.1\r\n\r\nGET /b").unwrap();
    assert_eq!(reqs.len(), 1);
    assert_eq!(p.buffered(), "GET /b".len());
    assert_eq!(p.feed(b" HTTP/1.1\r\n\r\n").unwrap().len(), 1);
    assert_eq!(p.buffered(), 0);
}

#[test]
fn r20_buffered_zero_after_each_complete_request() {
    let inputs: [&[u8]; 3] = [
        b"GET / HTTP/1.1\r\n\r\n",
        b"POST / HTTP/1.1\r\nContent-Length: 2\r\n\r\nhi",
        b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nhi\r\n0\r\nT: 1\r\n\r\n",
    ];
    let mut p = Parser::new(Limits::default());
    for input in inputs {
        assert_eq!(p.feed(input).unwrap().len(), 1);
        assert_eq!(p.buffered(), 0);
    }
}

// Incremental feeding
const CHUNKED_REQ: &[u8] = b"POST /up?x=1 HTTP/1.1\r\nHost: h\r\nTransfer-Encoding: chunked\r\nX-A:  v \r\n\r\n4;e=1\r\nWiki\r\n5\r\npedia\r\nE\r\n in\r\n\r\nchunks.\r\n0\r\nT: 1\r\n\r\n";

#[test]
fn feed_chunked_byte_at_a_time_equals_whole() {
    let whole = parse_all(CHUNKED_REQ).unwrap();
    assert_eq!(whole.len(), 1);
    assert_eq!(whole[0].body, b"Wikipedia in\r\n\r\nchunks.");
    let bytewise = parse_bytewise(Limits::default(), CHUNKED_REQ).unwrap();
    assert_eq!(whole, bytewise);
}

#[test]
fn feed_three_pipelined_requests() {
    let input = [
        &b"GET /1 HTTP/1.1\r\nHost: a\r\n\r\n"[..],
        b"POST /2 HTTP/1.1\r\nContent-Length: 4\r\n\r\nabcd",
        b"PUT /3 HTTP/1.0\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nxy\r\n0\r\n\r\n",
    ]
    .concat();
    let reqs = parse_all(&input).unwrap();
    assert_eq!(reqs.len(), 3);
    assert_eq!(reqs[0].target, "/1");
    assert_eq!(reqs[1].body, b"abcd");
    assert_eq!(reqs[2].version, Version::Http10);
    assert_eq!(reqs[2].body, b"xy");
}

#[test]
fn feed_split_at_every_byte_boundary() {
    let inputs: [&[u8]; 2] = [
        CHUNKED_REQ,
        b"POST / HTTP/1.1\r\nContent-Length: 5\r\nA: b\r\n\r\nhelloGET / HTTP/1.1\r\n\r\n",
    ];
    for input in inputs {
        let whole = parse_all(input).unwrap();
        for cut in 0..=input.len() {
            let mut p = Parser::new(Limits::default());
            let mut got = p.feed(&input[..cut]).unwrap();
            got.extend(p.feed(&input[cut..]).unwrap());
            assert_eq!(got, whole, "cut at {cut}");
            assert_eq!(p.buffered(), 0);
        }
    }
}

// Request smuggling and malformed input. Each row is checked whole and byte by byte.
#[test]
fn smuggling_table() {
    let cases: &[(&str, &[u8], ParseError)] = &[
        (
            "CL.TE",
            b"POST / HTTP/1.1\r\nContent-Length: 6\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\nG",
            ParseError::AmbiguousLength,
        ),
        (
            "TE.CL",
            b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\nContent-Length: 4\r\n\r\n12\r\nGPOST / HTTP/1.1\r\n\r\n0\r\n\r\n",
            ParseError::AmbiguousLength,
        ),
        (
            "TE.TE xchunked",
            b"POST / HTTP/1.1\r\nTransfer-Encoding: xchunked\r\n\r\n0\r\n\r\n",
            ParseError::UnsupportedTransferEncoding,
        ),
        (
            "TE with vertical tab prefix",
            b"POST / HTTP/1.1\r\nTransfer-Encoding: \x0bchunked\r\n\r\n0\r\n\r\n",
            ParseError::BadHeaderValue,
        ),
        (
            "space before colon",
            b"POST / HTTP/1.1\r\nTransfer-Encoding : chunked\r\n\r\n0\r\n\r\n",
            ParseError::BadHeaderName,
        ),
        (
            "folded TE",
            b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n gzip\r\n\r\n0\r\n\r\n",
            ParseError::ObsFold,
        ),
        (
            "two different CL",
            b"POST / HTTP/1.1\r\nContent-Length: 5\r\nContent-Length: 6\r\n\r\nhello!",
            ParseError::ConflictingContentLength,
        ),
        (
            "CL list",
            b"POST / HTTP/1.1\r\nContent-Length: 5, 5\r\n\r\nhello",
            ParseError::BadContentLength,
        ),
        (
            "CL with plus sign",
            b"POST / HTTP/1.1\r\nContent-Length: +5\r\n\r\nhello",
            ParseError::BadContentLength,
        ),
        (
            "CL wraps u64",
            b"POST / HTTP/1.1\r\nContent-Length: 18446744073709551621\r\n\r\nhello",
            ParseError::BadContentLength,
        ),
        (
            "bare LF request line",
            b"GET / HTTP/1.1\nTransfer-Encoding: chunked\r\n\r\n",
            ParseError::BareLf,
        ),
        (
            "bare LF header",
            b"GET / HTTP/1.1\r\nA: b\nContent-Length: 5\r\n\r\n",
            ParseError::BareLf,
        ),
        (
            "bare CR is not a line end",
            b"GET / HTTP/1.1\rTransfer-Encoding: chunked\r\n\r\n",
            ParseError::BadRequestLine,
        ),
        (
            "NUL in value",
            b"GET / HTTP/1.1\r\nA: b\x00c\r\n\r\n",
            ParseError::BadHeaderValue,
        ),
        (
            "chunk size with trailing space",
            b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n0 \r\n\r\n",
            ParseError::BadChunk,
        ),
        (
            "chunk size 17 digits",
            b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n00000000000000001\r\nx\r\n0\r\n\r\n",
            ParseError::BadChunk,
        ),
        (
            "chunk data longer than declared",
            b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n1\r\nabc\r\n0\r\n\r\n",
            ParseError::BadChunk,
        ),
        (
            "HTTP/0.9 style",
            b"GET /\r\n\r\n",
            ParseError::BadRequestLine,
        ),
        (
            "leading blank line",
            b"\r\nGET / HTTP/1.1\r\n\r\n",
            ParseError::BadRequestLine,
        ),
        (
            "header without colon",
            b"GET / HTTP/1.1\r\nContent-Length 5\r\n\r\n",
            ParseError::BadHeaderName,
        ),
    ];
    assert!(cases.len() >= 12);
    for (name, input, want) in cases {
        assert_eq!(parse_all(input), Err(*want), "whole: {name}");
        assert_eq!(
            parse_bytewise(Limits::default(), input),
            Err(*want),
            "bytewise: {name}"
        );
    }
}

// Property test: generator model vs parser output under random split points.
#[derive(Debug, Clone)]
enum BodyMode {
    Absent,
    Length,
    Chunked {
        cuts: Vec<usize>,
        ext: bool,
        trailer: bool,
    },
}

#[derive(Debug, Clone)]
struct Model {
    method: &'static str,
    target: String,
    http10: bool,
    headers: Vec<(String, String, String, String)>, // name, value, left pad, right pad
    body: Vec<u8>,
    mode: BodyMode,
}

impl Model {
    fn encode(&self) -> Vec<u8> {
        let v = if self.http10 { "HTTP/1.0" } else { "HTTP/1.1" };
        let mut out = format!("{} {} {}\r\n", self.method, self.target, v).into_bytes();
        for (n, val, l, r) in &self.headers {
            out.extend_from_slice(format!("{n}:{l}{val}{r}\r\n").as_bytes());
        }
        match &self.mode {
            BodyMode::Absent => out.extend_from_slice(b"\r\n"),
            BodyMode::Length => {
                out.extend_from_slice(
                    format!("Content-Length: {}\r\n\r\n", self.body.len()).as_bytes(),
                );
                out.extend_from_slice(&self.body);
            }
            BodyMode::Chunked { cuts, ext, trailer } => {
                out.extend_from_slice(b"Transfer-Encoding: chunked\r\n\r\n");
                let mut rest = &self.body[..];
                let mut cuts = cuts.iter();
                while !rest.is_empty() {
                    let n = (cuts.next().copied().unwrap_or(rest.len()) % rest.len()) + 1;
                    let (head, tail) = rest.split_at(n);
                    let e = if *ext { ";k=v" } else { "" };
                    out.extend_from_slice(format!("{:x}{e}\r\n", head.len()).as_bytes());
                    out.extend_from_slice(head);
                    out.extend_from_slice(b"\r\n");
                    rest = tail;
                }
                out.extend_from_slice(b"0\r\n");
                if *trailer {
                    out.extend_from_slice(b"X-Trailer: t\r\n");
                }
                out.extend_from_slice(b"\r\n");
            }
        }
        out
    }

    fn expected(&self) -> Request {
        let mut headers: Vec<(String, Vec<u8>)> = self
            .headers
            .iter()
            .map(|(n, v, _, _)| (n.clone(), v.clone().into_bytes()))
            .collect();
        match self.mode {
            BodyMode::Absent => {}
            BodyMode::Length => headers.push((
                "Content-Length".into(),
                self.body.len().to_string().into_bytes(),
            )),
            BodyMode::Chunked { .. } => {
                headers.push(("Transfer-Encoding".into(), b"chunked".to_vec()))
            }
        }
        Request {
            method: self.method.to_string(),
            target: self.target.clone(),
            version: if self.http10 {
                Version::Http10
            } else {
                Version::Http11
            },
            headers,
            body: self.body.clone(),
        }
    }
}

fn model_strategy() -> impl Strategy<Value = Model> {
    let method = prop::sample::select(vec![
        "GET", "POST", "PUT", "DELETE", "PATCH", "OPTIONS", "M-SEARCH", "custom",
    ]);
    let target = "/[a-z0-9/._~?=&%-]{0,20}";
    // The "X" prefix keeps generated names away from the framing headers.
    let name = "X[A-Za-z0-9-]{0,10}";
    let value = "([!-~]([ -~]{0,12}[!-~])?)?";
    let pad = prop::sample::select(vec!["", " ", "\t", "  \t"]).prop_map(String::from);
    let header = (name, value, pad.clone(), pad);
    let body = prop::collection::vec(any::<u8>(), 0..120);
    let mode = prop_oneof![
        Just(BodyMode::Absent),
        Just(BodyMode::Length),
        (
            prop::collection::vec(0usize..40, 0..6),
            any::<bool>(),
            any::<bool>()
        )
            .prop_map(|(cuts, ext, trailer)| BodyMode::Chunked { cuts, ext, trailer }),
    ];
    (
        method,
        target,
        any::<bool>(),
        prop::collection::vec(header, 0..6),
        body,
        mode,
    )
        .prop_map(|(method, target, http10, headers, body, mode)| {
            let body = if matches!(mode, BodyMode::Absent) {
                vec![]
            } else {
                body
            };
            Model {
                method,
                target,
                http10,
                headers,
                body,
                mode,
            }
        })
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 300,
        failure_persistence: None,
        // Fixed seed: the same 300 cases on every run.
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x68_74_74_70),
        ..ProptestConfig::default()
    })]

    #[test]
    fn prop_parser_matches_model(
        models in prop::collection::vec(model_strategy(), 1..4),
        splits in prop::collection::vec(any::<prop::sample::Index>(), 0..8),
    ) {
        let wire: Vec<u8> = models.iter().flat_map(Model::encode).collect();
        let mut points: Vec<usize> = splits.iter().map(|i| i.index(wire.len() + 1)).collect();
        points.sort_unstable();
        points.push(wire.len());

        let mut p = Parser::new(Limits::default());
        let mut got = Vec::new();
        let mut prev = 0;
        for end in points {
            got.extend(p.feed(&wire[prev..end]).unwrap());
            prev = end;
        }
        let want: Vec<Request> = models.iter().map(Model::expected).collect();
        prop_assert_eq!(got, want);
        prop_assert_eq!(p.buffered(), 0);
    }
}
