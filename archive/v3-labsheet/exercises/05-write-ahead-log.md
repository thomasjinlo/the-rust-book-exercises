# Exercise 05: Write-ahead log

**Time:** 3 h | **Crate:** `crates/ex05-write-ahead-log`

## Read first
**Rust Book**
- [Ch. 4.2: References and Borrowing](https://doc.rust-lang.org/book/ch04-02-references-and-borrowing.html)
- [Ch. 4.3: The Slice Type](https://doc.rust-lang.org/book/ch04-03-slices.html)
- [Ch. 5: Using Structs to Structure Related Data](https://doc.rust-lang.org/book/ch05-00-structs.html)
- [Ch. 5.3: Methods](https://doc.rust-lang.org/book/ch05-03-method-syntax.html)
- [Ch. 6.2: The match Control Flow Construct](https://doc.rust-lang.org/book/ch06-02-match.html)
- [Ch. 8.1: Storing Lists of Values with Vectors](https://doc.rust-lang.org/book/ch08-01-vectors.html)
- [Ch. 9.2: Recoverable Errors with Result](https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html)
- [Ch. 9.3: To panic! or Not to panic!](https://doc.rust-lang.org/book/ch09-03-to-panic-or-not-to-panic.html)
- [Ch. 11.1: How to Write Tests](https://doc.rust-lang.org/book/ch11-01-writing-tests.html)
- [Ch. 11.3: Test Organization](https://doc.rust-lang.org/book/ch11-03-test-organization.html)
- [Ch. 12.2: Reading a File](https://doc.rust-lang.org/book/ch12-02-reading-a-file.html)

**Standard library**
- [`Path`](https://doc.rust-lang.org/std/path/struct.Path.html)
- [`ErrorKind`](https://doc.rust-lang.org/std/io/enum.ErrorKind.html)

**Background**
- [Write Ahead Log File Format (RocksDB wiki)](https://github.com/facebook/rocksdb/wiki/Write-Ahead-Log-File-Format): how a storage engine lays out log records (checksum, length, payload) and how a reader treats a damaged record.
- [28.1. Reliability (PostgreSQL documentation)](https://www.postgresql.org/docs/current/wal-reliability.html): why a power loss leaves a partly written log tail and how a per-record checksum lets crash recovery detect it.

## Task
Build an append-only log file. Each call to `append` adds one record. After a crash cuts the file short or a bit
in it flips, `open` and `read_all` must return only records that are whole and unchanged. You choose the file
format. The tests only cut the file, flip bits in it and compare what the API returns.

## API
A record is one `payload` passed to `append`. It may be empty. `max_record_len` is the largest allowed payload in
bytes. No call may panic, for any file content.

### `Recovery`
```rust
pub struct Recovery {
    pub records_recovered: u64,
}
```
Derives `Debug, Clone, Copy, PartialEq, Eq`. The starter already has it.

### `Wal::open`
```rust
pub fn open(path: &Path, max_record_len: u32) -> io::Result<(Wal, Recovery)>
```
A record is **valid** when all of these hold:
- Every byte that `append` wrote to the file for it is present.
- None of those bytes changed.
- Its payload is at most `max_record_len` bytes. This is checked on every `open` and `read_all`, with the limit
  passed to that call.

The **kept records** of a file are its valid records up to, but not including, the first record that is not
valid. Records after that one are dropped even if they are valid. A file is **damaged** when it holds anything
besides its kept records.

| File | Returns |
|---|---|
| Path does not exist | The file is created. `Ok`, `records_recovered: 0` |
| Empty file | `Ok`, `records_recovered: 0` |
| Log file cut to any length from 0 to its full length | `Ok`, `records_recovered` = number of kept records |
| Log file with one bit flipped at byte 16 or later | `Ok`, `records_recovered` = number of kept records |
| Log file with one bit flipped in bytes 0 to 15 | `Ok` as above, or any `Err` |
| File of 64 bytes or more that this log did not write, such as a text file | `Err` with `ErrorKind::InvalidData` |

1. `records_recovered` is the number of kept records. The `Wal` holds exactly those records.
2. A file that is not damaged keeps its bytes unchanged.
3. A damaged file is repaired by `open`. Afterwards, `append` adds a record right after the last kept record.
   A later `open` or `read_all` returns the kept records, then every record appended since, in order.
4. An `Err` from `InvalidData` leaves the file unchanged.
5. A file smaller than 1 KiB, opened with `max_record_len = u32::MAX`, never makes `open` or `read_all` request
   one allocation of 1 MiB or more, whichever bit is flipped.

### `Wal::append`
```rust
pub fn append(&mut self, payload: &[u8]) -> io::Result<u64>
```
| Input | Returns |
|---|---|
| `payload.len() <= max_record_len` | `Ok(sequence number)` |
| `payload.len() > max_record_len` | `Err` with `ErrorKind::InvalidInput` |

1. The first record of a log has sequence number 0. Each next record has the previous number plus 1, also across
   `open` calls.
2. After `open`, the next `append` returns `records_recovered`.
3. When `append` returns `Ok`, the record is already in the file. Another caller of `Wal::read_all(path, ..)`
   sees it, and the file is longer than before the call. The `Wal` stays open.
4. A failed `append` does not change the file and does not use a sequence number.

### `Wal::read_all`
```rust
pub fn read_all(path: &Path, max_record_len: u32) -> io::Result<Vec<Vec<u8>>>
```
| File | Returns |
|---|---|
| Path does not exist | `Err` with `ErrorKind::NotFound`. The file is not created. |
| Empty file | `Ok(vec![])` |
| Any other file | The same `Ok`/`Err` as `open` gives for it |

1. On `Ok`, returns the payloads of the kept records, in order.
2. Never changes the file, even when it is damaged.

## Provided (do not edit)
`Cargo.toml` lists `crc32fast`, a checksum crate you may use, and `tempfile` for tests.

## Example
Use `max_record_len = 1024`.

| Call | Returns |
|---|---|
| `Wal::open(path, 1024)`, path does not exist | `Ok`, `records_recovered: 0` |
| `append(b"alpha")` | `Ok(0)` |
| `append(b"")` | `Ok(1)` |
| `append(b"gamma")` | `Ok(2)` |
| `append(&[0; 1025])` | `Err(InvalidInput)` |
| drop the log, `Wal::open(path, 1024)` | `Ok`, `records_recovered: 3` |
| `append(b"delta")` | `Ok(3)` |
| drop the log, `Wal::read_all(path, 1024)` | `Ok([b"alpha", b"", b"gamma", b"delta"])` |
| `Wal::read_all(path, 5)` | same four records |
| `Wal::read_all(path, 4)` | `Ok([])`, because `alpha` is 5 bytes and ends the kept records |

Start from the file left by the calls above (four records). Damage a copy of it, then call
`Wal::open(path, 1024)`:

| Damage | `records_recovered` | `read_all` after `open` | Next `append` |
|---|---|---|---|
| Cut inside `delta` | 3 | `[alpha, "", gamma]` | `Ok(3)` |
| Cut inside `gamma` | 2 | `[alpha, ""]` | `Ok(2)` |
| Cut to 0 bytes | 0 | `[]` | `Ok(0)` |
| One bit flipped in `gamma`'s payload | 2 | `[alpha, ""]` | `Ok(2)` |
| One bit flipped in `alpha`'s payload | 0 | `[]` | `Ok(0)` |
| 70 bytes of text instead of a log | `Err(InvalidData)` | `Err(InvalidData)` | none |

## Done when
Edit only `src/lib.rs`. Do not change the public signatures or edit `tests/`. Add your own tests in
`tests/extra.rs`. The starter compiles and all 12 of its tests fail until you finish.

From `crates/ex05-write-ahead-log`, all of these exit 0:
```
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```
`cargo test` reports 12 passed. It takes about 7 seconds.

## Extra tests (optional)
- 1,000 records of random length 0 to 2,000, file cut at 50 random lengths -> `records_recovered` equals the number of records whose bytes are all still present.
- 30 records, one bit flipped in record 10 and one in record 20 -> records 0 to 9 come back, nothing else.
- Append 4,096 zero bytes to a valid log -> exactly the original records come back.
- Record of exactly `max_record_len` bytes -> comes back with the same limit, is dropped with the limit one lower.
- `Wal::open` on a path that is a directory -> `Err`, no panic.
- First `append` is too long, second is valid -> the second returns `Ok(0)`.

## Stretch (optional)
- Add a way to force a record to disk before `append` returns, and measure the cost per record. Compare with forcing once per batch.
- Let several threads append to one log. Sequence numbers must be 0 to N-1 with no gaps or repeats.
- Start a new file when the log passes a size you choose, and add `read_from(sequence_number)`.
