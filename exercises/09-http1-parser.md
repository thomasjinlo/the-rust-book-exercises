# Exercise 09: Incremental HTTP/1.1 request parser

**Time:** ~4 h | **Area:** networking / protocol parsing / security | **Crate:** `crates/ex09-http1-parser`
**Builds on:** 03 (incremental decoding idea; optional, never required)

## Read first
- Rust Book: [ch06 Enums](https://doc.rust-lang.org/book/ch06-00-enums.html), [ch08 Collections](https://doc.rust-lang.org/book/ch08-00-common-collections.html), [ch09 Error handling](https://doc.rust-lang.org/book/ch09-00-error-handling.html), [ch11 Testing](https://doc.rust-lang.org/book/ch11-00-testing.html), [ch13.2 Iterators](https://doc.rust-lang.org/book/ch13-02-iterators.html)
- RFC 9112: [message format](https://www.rfc-editor.org/rfc/rfc9112#name-message-format), [message body length](https://www.rfc-editor.org/rfc/rfc9112#name-message-body-length), [chunked transfer coding](https://www.rfc-editor.org/rfc/rfc9112#name-chunked-transfer-coding). Skim the request line and field syntax sections in the same RFC.
- RFC 9110 [tokens](https://www.rfc-editor.org/rfc/rfc9110#name-tokens) (the `tchar` set)
- [HTTP request smuggling](https://portswigger.net/web-security/request-smuggling): why two parsers that disagree on where a request ends are exploitable.

## What you are building
A parser for HTTP/1.1 requests that a server calls with whatever bytes the socket returned. It buffers partial input, returns every request that is complete (including pipelined ones), and rejects anything ambiguous. Reverse proxies and origin servers must agree on message boundaries. When they do not, an attacker hides a second request inside the body of the first. Your parser therefore refuses every input that has two plausible readings. It has no dependencies and does no I/O.

## Starting point
- Edit only `crates/ex09-http1-parser/src/lib.rs`: implement `Parser::new`, `Parser::feed`, `Parser::buffered`. Add private fields and private helper functions as you like. Do not change any public item.
- Items marked "Provided" (`Limits`, `Version`, `Request`, `Request::header`, `ParseError`) are complete. Do not edit them.
- Do not edit `tests/http1.rs` or `Cargo.toml`. Put your own tests in a new file under `tests/`.
- Run from `crates/ex09-http1-parser`: `cargo test`. On the unmodified starter all 49 given tests fail and the run ends in under a second.
- A simple design is enough: append to a buffer, then try to parse one request from the start of the buffer, repeat until it says "need more input". Re-parsing from the start on every call is acceptable here. The stretch section asks for a resumable version.

## Requirements
Errors are `ParseError` variants. "Rejected" means `feed` returns that variant.

**Request line**
- R1. The request line is `method SP target SP version` with exactly one SP between parts. Any other part count is `BadRequestLine`. A method is a non-empty token (RFC 9110 `tchar`) and keeps its case. A non-token method is `BadRequestLine`.
- R2. The target is non-empty and every byte is in `0x21..=0x7e`. Anything else (empty, control byte, HTAB, non-ASCII) is `BadRequestLine`. A leading blank line before the request line is also `BadRequestLine`.
- R3. The version is exactly `HTTP/1.0` or `HTTP/1.1`. Any other third part (`HTTP/1.2`, `http/1.1`, `HTTP/1.10`, empty) is `BadVersion`. Check order: part count, method, target, version.

**Lines and fields**
- R4. Every line ends in CRLF. An LF not preceded by CR is `BareLf`, in the request line, header lines, the blank line, chunk-size lines and trailers. A lone CR is not a line end: it is ordinary content of the line it appears in.
- R5. A header name is a non-empty token. A line with no colon, an empty name, a non-token name, or whitespace before the colon is `BadHeaderName`. Names keep their case. `Request::header` is ASCII case-insensitive and returns the first match.
- R6. A value has leading and trailing SP and HTAB removed, nothing else. An empty value is allowed. A value containing a byte below `0x20` other than HTAB, or `0x7f`, is `BadHeaderValue`. Bytes `0x80..=0xff` are allowed.
- R7. A header line that starts with SP or HTAB (obsolete line folding) is `ObsFold`.
- R8. Limits, all detected as soon as the data seen so far exceeds them, with or without the terminating CRLF:
  - Request line content longer than `max_request_line` (CRLF excluded) is `RequestLineTooLong`. A trailing CR with no LF yet is not counted.
  - Header lines count their content plus 2 bytes of CRLF each. The blank line does not count. A total above `max_header_bytes` is `HeaderTooLarge`.
  - More than `max_headers` header lines is `TooManyHeaders`, reported when the line that exceeds the limit is complete.

**Body length** (check order: R9, R10, R11, R13)
- R9. `Content-Length` is one or more ASCII digits after trimming. A sign, inner whitespace, a comma list, an empty value, or a value that does not fit in `u64` is `BadContentLength`. Leading zeros are fine. The body is exactly that many bytes. The request is not complete until all of them have arrived.
- R10. Several `Content-Length` headers with different values are `ConflictingContentLength`. Identical values are accepted and count as one. All values are checked for syntax (R9) before they are compared.
- R11. Both `Content-Length` and `Transfer-Encoding` present, in either order, is `AmbiguousLength`.
- R12. `Transfer-Encoding` selects chunked decoding when its last coding is `chunked` (ASCII case-insensitive). Values from several `Transfer-Encoding` headers are joined in order, and each value is split on commas and trimmed. Decoding rules:
  - Chunk size is hex digits followed optionally by `;` and an extension. The extension is ignored.
  - Data is followed by exactly CRLF.
  - A chunk of size 0 is followed by a trailer section, parsed with the header rules (R5 to R8) and discarded.
- R13. Any other `Transfer-Encoding` is `UnsupportedTransferEncoding`: last coding not `chunked`, an empty element (`chunked,`), a non-token element, or `chunked` appearing more than once. Codings before `chunked` (for example `gzip`) are accepted but not applied: the body holds the still-encoded bytes.
- R14. A chunk size has 1 to 16 hex digits and nothing else (no sign, no `0x`, no spaces). Otherwise it is `BadChunk`. So are a missing or wrong CRLF after chunk data (detected as soon as the first wrong byte arrives), and a chunk-size line (extension included) longer than `max_request_line`. Sixteen digits fit in `u64`, so overflow cannot occur once the length check passes.
- R15. A decoded body over `max_body` is `BodyTooLarge`, reported as soon as it is known: for `Content-Length` when the headers are complete, for chunked as soon as a chunk-size line makes the running total exceed the limit, before its data arrives. A body of exactly `max_body` is fine.
- R16. A request with neither header has an empty body, and the next request starts right after the blank line.
- R17. `HTTP/1.0` and `HTTP/1.1` requests parse identically apart from `Request::version`.
- R18. `Host` is not validated: it may be missing, duplicated or malformed. A caller that needs RFC 9112 host rules must check it itself.

**Parser behavior**
- R19. After any error the parser is poisoned: every later `feed`, including `feed(b"")`, returns the same error. Requests completed earlier in the same call are discarded. The buffer is released, so `buffered()` is 0.
- R20. `feed` returns all requests completed by its input, in order. Incomplete input stays buffered. `buffered()` is the number of bytes held for an incomplete request, and 0 right after a request completes when nothing else was fed.

Classic attacks covered by these rules: CL.TE and TE.CL disagreement (R11), obfuscated `Transfer-Encoding` such as `xchunked`, a leading `\x0b`, or a space before the colon (R5, R6, R13), folded headers (R7), duplicate or signed lengths (R9, R10), and bare LF line ends that a front end and back end treat differently (R4).

## Given tests
All in `tests/http1.rs`. The table lists each test function. "Total" is 49 tests.

| Test | Requirement |
|---|---|
| `r01_method_tokens_keep_case`, `r01_bad_method` | R1 |
| `r02_target_accepted`, `r02_target_rejected` | R2 |
| `r03_versions` | R3 |
| `r04_bare_lf` | R4 |
| `r05_header_names`, `r05_header_lookup_is_first_match_case_insensitive` | R5 |
| `r06_values_trim_ows`, `r06_value_control_bytes` | R6 |
| `r07_obs_fold` | R7 |
| `r08_request_line_limit`, `r08_request_line_limit_detected_without_crlf`, `r08_header_bytes_limit`, `r08_header_bytes_detected_without_terminator`, `r08_too_many_headers` | R8 |
| `r09_content_length_body`, `r09_bad_content_length`, `r09_waits_for_full_body` | R9 |
| `r10_conflicting_content_length`, `r10_identical_content_length_accepted` | R10 |
| `r11_ambiguous_length` | R11 |
| `r12_chunked_basic`, `r12_chunked_is_case_insensitive_and_hex`, `r12_chunk_extensions_ignored`, `r12_trailers_discarded`, `r12_trailers_are_validated` | R12 |
| `r13_unsupported_transfer_encoding`, `r13_chunked_last_after_other_coding_is_decoded_not_unwrapped`, `r13_split_transfer_encoding_headers` | R13 |
| `r14_bad_chunk_sizes`, `r14_sixteen_digits_parse_then_hit_body_limit`, `r14_bad_chunk_terminator`, `r14_chunk_size_line_limit` | R14 |
| `r15_content_length_over_limit_is_immediate`, `r15_chunked_total_over_limit` | R15 |
| `r16_no_body_headers_means_empty_body` | R16 |
| `r17_http10_and_http11_parse_alike` | R17 |
| `r18_host_is_not_validated` | R18 |
| `r19_error_poisons_parser`, `r19_error_discards_requests_from_same_feed` | R19 |
| `r20_buffered_tracks_partial_input`, `r20_buffered_keeps_start_of_next_request`, `r20_buffered_zero_after_each_complete_request` | R20 |
| `feed_chunked_byte_at_a_time_equals_whole` | R12, R20: one byte per `feed` equals one whole `feed` |
| `feed_three_pipelined_requests` | R20: three requests (one per body type) in one `feed` |
| `feed_split_at_every_byte_boundary` | R20: every two-way split of two inputs gives the whole-input result |
| `smuggling_table` | 20 malformed inputs, each expecting one specific error, fed whole and one byte at a time |
| `prop_parser_matches_model` | proptest, 300 cases with a fixed seed: 1 to 3 pipelined generated requests (random method, target, header names, values and padding, body, Content-Length or chunked with random chunk sizes, extensions and trailers), random split points, output equals the generator's model and `buffered()` is 0 |

## Acceptance
From `crates/ex09-http1-parser`:

```
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Pass condition: all three exit 0, and `cargo test` reports `49 passed; 0 failed` for `tests/http1.rs`. Run `cargo test` 20 times in a row (`for i in $(seq 20); do cargo test || break; done`); it must never fail. The proptest seed is fixed, so a failure is a real bug in your code. Optionally record the time of `cargo test --release` in your notes.

## Extra tests to write
1. Request line one byte over `max_request_line` while the CRLF is still missing. Expect `RequestLineTooLong` before the next byte arrives. Catches checking limits only on complete lines.
2. `Content-Length: 5` with 7 body bytes followed by `GET`. Expect a 5-byte body, then the next request parsed from byte 6. Catches over-reading the body.
3. `Transfer-Encoding: chunked` header, then headers split exactly between `\r` and `\n` of the blank line. Expect one request. Catches losing a lone trailing CR.
4. Two `Transfer-Encoding: chunked` headers. Expect `UnsupportedTransferEncoding`. Catches joining headers without the duplicate check.
5. Chunked request with 1000 one-byte chunks and `max_body = 999`. Expect `BodyTooLarge` at the chunk-size line of the 1000th chunk. Catches checking the total only at the end.
6. A valid request, then a bad one, fed byte by byte. Expect the first request returned in its call and the error on the byte that completes the bad line. Catches batching.
7. Header name with a trailing `\0` or a UTF-8 letter. Expect `BadHeaderName`. Catches `char::is_alphanumeric` instead of ASCII.
8. `max_header_bytes = 0` and `max_headers = 0`. Expect any header line to be rejected and a request with no headers to parse. Catches underflow in budget arithmetic.

## Stretch (optional)
- Make the parser resumable. Store a state (request line, headers, body, chunk size, chunk data, trailers) and the parsed prefix, so each byte is examined once. Measure: feeding a 1 MiB chunked body one byte at a time should take time linear in its size.
- Zero-copy parse. Return a `RequestRef<'a>` whose method, target, header names and values are `&'a [u8]` ranges into the caller's buffer, and copy only when a request spans two `feed` calls.
- Add a response parser with the same rules (status line, body length rules from RFC 9112 for responses, including read-until-close). Then add a fuzz target. Commands only, nothing to run in the given tests: `cargo install cargo-fuzz`, `cargo fuzz init`, a target that feeds random split points of arbitrary bytes into `Parser::feed` and asserts that whole-input and byte-at-a-time feeding give the same result, then `cargo +nightly fuzz run <target> -- -max_total_time=300`.
