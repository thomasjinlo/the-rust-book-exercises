# Exercise 05: Write-ahead log

**Time:** 2 h | **Crate:** `crates/ex05-write-ahead-log`

## Task
Build an append-only log file. After a crash cuts the file short or a bit in it flips, `open` returns only
records that are whole and unchanged. You choose the file format; the tests only cut the file, flip bits in it
and compare what `open` returns.

## Example
A record is a byte string and may be empty. The path does not exist at first.

| Call | Returns |
|---|---|
| `Wal::open(path)` | `Ok((wal, []))`, the file is created |
| `append(b"alpha")`, `append(b"")`, `append(b"gamma")` | `Ok(())` each |
| drop `wal`, `Wal::open(path)` | `Ok((wal, [alpha, "", gamma]))` |

Damage a copy of that file, then call `Wal::open(path)`:

| Damage | Records returned |
|---|---|
| Cut inside `gamma` | `[alpha, ""]` |
| Cut to 0 bytes | `[]` |
| One bit flipped in `gamma` | `[alpha, ""]` |
| One bit flipped in `alpha` | `[]`, because `""` and `gamma` come after a damaged record |

## API
Create or open a log with `Wal::open`.

### `open`
```rust
pub fn open(path: &Path) -> io::Result<(Wal, Vec<Vec<u8>>)>
```
1. A missing file is created. The result holds the records recovered from the file, in the order they were appended.
2. After the file is cut at any length or one bit is flipped, the result is an exact prefix of the appended
   records: never a changed record, never a record after a damaged one.
3. After such an `open`, `append` works, and a later `open` returns the recovered prefix plus the new records.

### `append`
```rust
pub fn append(&mut self, record: &[u8]) -> io::Result<()>
```
1. Returns after the record is in the file, so a new `open` on the same path already sees it.
2. The record may be empty or hold any bytes.

## Done when
Edit only `src/lib.rs`. From `crates/ex05-write-ahead-log`, `cargo test` reports 9 passed (the starter fails all 9).
One test makes a damaged copy for every cut length from 0 to the full file size, and one for every single bit of
the file flipped. Each time `open` must return a prefix of the appended records and a later `append` must work. `Cargo.toml` lists `crc32fast`, a
checksum crate you may use.

## Read first
- [Ch. 9.2: Recoverable Errors with Result](https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html)
- [Ch. 12.2: Reading a File](https://doc.rust-lang.org/book/ch12-02-reading-a-file.html)
- [Write Ahead Log File Format (RocksDB wiki)](https://github.com/facebook/rocksdb/wiki/Write-Ahead-Log-File-Format)

## Stretch (optional)
- Reject records over a size limit you choose, and make sure a damaged length cannot make `open` allocate that much.
- Add a way to force a record to disk before `append` returns, and measure the cost per record.
