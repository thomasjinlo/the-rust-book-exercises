# Exercise 05: Write-ahead log with crash recovery

**Time:** ~3 h | **Area:** systems programming / storage / durability | **Crate:** `crates/ex05-write-ahead-log`
**Builds on:** none (standalone)

## Read first
- Rust Book ch04, ownership and slices: https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html
- Rust Book ch05, structs: https://doc.rust-lang.org/book/ch05-00-structs.html
- Rust Book ch06, enums and `match`: https://doc.rust-lang.org/book/ch06-00-enums.html
- Rust Book ch08, `Vec`: https://doc.rust-lang.org/book/ch08-00-common-collections.html
- Rust Book ch09, error handling (`io::Result`, `?`): https://doc.rust-lang.org/book/ch09-00-error-handling.html
- Rust Book ch10, generics and traits (`impl Read`, `impl FnMut`): https://doc.rust-lang.org/book/ch10-00-generics.html
- Rust Book ch11, testing: https://doc.rust-lang.org/book/ch11-00-testing.html
- Rust Book ch12, file I/O in a real program: https://doc.rust-lang.org/book/ch12-00-an-io-project.html
- `std::fs::File` (`set_len`, `sync_data`, `metadata`): https://doc.rust-lang.org/std/fs/struct.File.html
- `std::io::Seek` and `SeekFrom`: https://doc.rust-lang.org/std/io/trait.Seek.html
- `crc32fast`: https://docs.rs/crc32fast/latest/crc32fast/

You also use `std::fs::OpenOptions` (read, write, create, no truncate) and `Read::read_exact`. Their docs are one page each.

## What you are building
A write-ahead log (WAL) is an append-only file that a database or queue writes before it applies a change. After a crash it replays the file to rebuild its state. A crash can leave the last record half written, so the log must detect that when it is opened, cut the damage off and carry on appending.

You build `Wal`: a log of length-prefixed, checksummed records. `open` scans the file, truncates it after the last intact record and reports what it did. `append` adds a record and returns its sequence number (LSN). The API is single-threaded.

## Concepts

**Record format.** Every record stores its payload length, a CRC-32 of the payload, then the payload. A file is a fixed 8-byte header followed by records. All integers are little-endian.

```text
offset  size  field
0       8     header: the ASCII bytes "WALv0001"
8       4     record 0: payload_len (u32)
12      4     record 0: crc32 of the payload (u32)
16      n     record 0: payload (n = payload_len bytes)
16+n    4     record 1: payload_len
...           and so on until end of file
```

**A real file.** Appending `"hello"`, `"wal"` and `"ok"` to a new log produces 42 bytes. This dump comes from running the reference solution. Each row is 8 bytes; the annotations say which record field each byte belongs to.

```text
off  bytes                    meaning
  0  57 41 4c 76 30 30 30 31  header "WALv0001"
  8  05 00 00 00 86 a6 10 36  record 0: payload_len=5, crc=0x3610a686 (CRC-32 of "hello")
 16  68 65 6c 6c 6f 03 00 00  record 0: payload "hello" | record 1 starts at 21: payload_len=3 (3 of 4 bytes)
 24  00 52 d0 1d 96 77 61 6c  payload_len last byte | crc=0x961dd052 | payload "wal"
 32  02 00 00 00 47 dd dc 79  record 2: payload_len=2, crc=0x79dcdd47
 40  6f 6b                    record 2: payload "ok"
 42  (end of file)
```

Record 0 covers offsets 8..21, record 1 covers 21..32 and record 2 covers 32..42. A number is stored low byte first, so `86 a6 10 36` is `0x3610a686`. The next record always starts at `this start + 8 + payload_len`.

**The same file cut mid-record.** Suppose the process dies after the OS wrote only the first 30 bytes. The file is the dump above up to offset 30:

```text
 16  68 65 6c 6c 6f 03 00 00  (unchanged)
 24  00 52 d0 1d 96 77        record 1: header complete, payload_len=3, only 1 of 3 payload bytes ("w")
 30  (end of file)
```

Recovery reads record 1's header, sees that 3 payload bytes are needed and only 1 remains, and stops. It calls `set_len(21)`, so the file ends exactly after record 0. The reference solution returns `Recovery { records_recovered: 1, bytes_truncated: 9 }` for this file. The next `append` writes at offset 21 and returns LSN 1.

**What `sync_data` guarantees.** `write_all` only hands bytes to the operating system; after a power loss they may be gone. `File::sync_data` returns once the file's data has reached the disk, so every record written before a completed `sync_data` survives a crash, and records written after it may or may not.

**Why recovery stops at the first bad record.** A log is the history of changes in order. If record 5 is damaged but record 6 is intact, replaying record 6 without record 5 applies a change on top of a state that never saw its predecessor. So recovery keeps the longest valid prefix and discards everything after the first invalid record, intact or not.

**A record is invalid** when any of these holds: fewer than 8 bytes remain for its header, `payload_len` is larger than `max_record_len`, fewer than `payload_len` bytes remain, or the CRC of the payload does not match. Compare `payload_len` with the remaining file size before you allocate a buffer. A corrupt length of `0xFFFFFFFF` must not allocate 4 GiB.

**LSN.** The LSN of a record is its 0-based position in the file. After opening a file with `k` valid records the next append returns LSN `k`.

## Milestones
Run the filter after each stage. Tests outside the filter may still fail.

1. **Header and `open`** (R1 to R3). Open or create the file, write and sync the header for a new or short file, reject a wrong header. Use `OpenOptions::new().read(true).write(true).create(true).truncate(false)`, then `metadata().len()`. For a valid header return `Recovery { records_recovered: 0, bytes_truncated: 0 }` for now.
   Run `cargo test header_handling`. It turns fully green after stage 2, because it also calls `append`.
2. **`append` and `read_all`** (R4, R5, R16, R17). Build the 8-byte prefix and the payload in one `Vec<u8>` and `write_all` it. Write a helper `scan` that walks the records from offset 8 and stops at the first invalid one, and build `read_all` on it. Seek to the end of the valid data after `open`, before the first append.
   Run `cargo test -- header_handling on_disk_format append_returns read_all_on_short`. Green: `header_handling`, `on_disk_format_is_little_endian_len_crc_payload`, `append_returns_lsns_and_read_all_returns_payloads_in_order`, `read_all_on_short_or_missing_file`.
3. **Reopen and continue LSNs** (R11). Make `open` run `scan` on an existing file and set the next LSN to the number of records found.
   Run `cargo test lsns_continue`. Green: `lsns_continue_after_reopen`.
4. **Truncated tail** (R7, R8). When `scan` ends before the end of the file, call `set_len` and `sync_data`, and fill in `bytes_truncated`.
   Run `cargo test -- truncation read_all_stops`. Green: `truncation_at_every_byte_of_the_last_record_recovers_the_prefix`, `read_all_stops_at_damage_and_leaves_the_file_alone`.
5. **Corruption** (R7, R9, R10). Check the CRC, and compare the length field with the remaining file size before you allocate the payload buffer.
   Run `cargo test -- payload_bit_flip length_field`. Green: `payload_bit_flip_discards_the_record_and_everything_after_it`, `length_field_larger_than_the_file_truncates_there`.
6. **Sync counting and size limit** (R6, R12 to R15). Add the counter, `sync`, the `sync_each_append` option and the `max_record_len` checks in `append` and `scan`.
   Run `cargo test -- sync max_record`. Green: `sync_each_append_counts_one_sync_per_append`, `without_sync_each_append_only_explicit_sync_counts`, `max_record_len_is_enforced_on_append`, `record_longer_than_max_record_len_is_invalid_on_open`.

Finish with a plain `cargo test`. The commands with several names use `--` because `cargo test` takes only one filter before it.

## Starting point
- Edit only `crates/ex05-write-ahead-log/src/lib.rs`. The public items, their doc comments and the constants `FILE_HEADER` and `RECORD_HEADER_LEN` are the contract. Add private fields and helpers freely.
- Do not edit `crates/ex05-write-ahead-log/tests/wal.rs` or `Cargo.toml`. Add your own tests in a separate file such as `tests/extra.rs`.
- Dependencies are already listed: `crc32fast` (CRC-32), `tempfile` (dev only).
- Run: `cd crates/ex05-write-ahead-log && cargo test`.

On the unmodified starter, `cargo test` finishes in under a second with 13 failures.

## Requirements

On-disk format, all integers little-endian:

```text
file   := "WALv0001" record*
record := u32 payload_len | u32 crc32(payload) | payload bytes
```

`crc32` is `crc32fast::hash(payload)`. An empty payload is a valid record of 8 bytes (its CRC is 0).

**Opening**

R1. `Wal::open(path, opts)` returns `io::Result<(Wal, Recovery)>`. It creates the file if missing. `WalOptions { sync_each_append: bool, max_record_len: u32 }` is the only configuration.
R2. A file that is missing, empty, or shorter than 8 bytes gets the 8-byte header written and synced. Any bytes already there are discarded. The result is `Recovery { records_recovered: 0, bytes_truncated: 0 }`.
R3. A file of 8 or more bytes whose first 8 bytes are not `WALv0001` makes `open` return `ErrorKind::InvalidData`. The file is not modified.

**Appending**

R4. `append(&mut self, payload) -> io::Result<u64>` returns the record's 0-based LSN. LSNs are gap-free and equal the record's position in the file.
R5. Each record reaches the file through one `write_all` call on a buffer that holds the 8-byte record prefix and the payload.
R6. `payload.len() > max_record_len` returns `ErrorKind::InvalidInput`, writes nothing, syncs nothing and consumes no LSN. A payload of exactly `max_record_len` bytes is accepted.

**Recovery**

R7. A record is valid when all `8 + payload_len` bytes are in the file, `payload_len <= max_record_len` and the CRC matches.
R8. `open` scans from offset 8. At the first invalid or incomplete record it calls `File::set_len` with the end offset of the last valid record, then `sync_data`. It sets `bytes_truncated` to the number of bytes removed and `records_recovered` to the number of records kept. A clean file has `bytes_truncated == 0` and no `set_len`.
R9. Records after the first invalid record are discarded even if they are intact.
R10. A length field that points past the end of the file is rejected before any buffer of `payload_len` bytes is allocated.
R11. After `open` the next LSN equals `records_recovered`, and appends go to the end of the recovered file.
R12. `max_record_len` also applies on recovery. Reopening with a smaller limit makes longer records invalid, and so truncated per R8 and R9.

**Syncing**

R13. With `sync_each_append == true`, `append` calls `sync_data` once after a successful write and before it returns. With `false`, `append` never syncs.
R14. `sync(&mut self)` calls `sync_data` once.
R15. `sync_count(&self)` returns the number of `sync_data` calls made through `append` and `sync` on this handle. Calls made inside `open` are not counted. A new handle starts at 0.

**Reading**

R16. `Wal::read_all(path, max_record_len) -> io::Result<Vec<Vec<u8>>>` returns the valid records in file order and stops at the first invalid or incomplete one. It never modifies the file.
R17. `read_all` on a missing file returns `ErrorKind::NotFound`. A file shorter than 8 bytes gives `Ok(vec![])`. A file of 8 or more bytes with a wrong header gives `ErrorKind::InvalidData`.

## Common mistakes
- Allocating `vec![0; payload_len]` before checking `payload_len` against the remaining file size. A flipped high bit in the length then allocates gigabytes.
- Treating a short read as an I/O error. `read_exact` returns `UnexpectedEof` when a record is cut off. That means "stop scanning", not "fail `open`".
- Forgetting to seek to the end of the recovered data. The cursor is wherever the scan left it. Seek to the recovered end before the first `append`, or new records land in the wrong place.
- Using `File::create` or `.truncate(true)` in `open`. Both erase an existing log.
- Writing the 8-byte prefix and the payload with separate `write_all` calls. A crash between them leaves a header with no payload.
- Updating the LSN counter or `sync_count` before the write or the size check succeeds.

## Given tests
All in `tests/wal.rs`. 13 tests, all run by default.

| Test | Requirement |
|---|---|
| `header_handling` | R2, R3, R17 |
| `on_disk_format_is_little_endian_len_crc_payload` | format, R5 |
| `append_returns_lsns_and_read_all_returns_payloads_in_order` | R4, R7, R16 (including empty payloads) |
| `read_all_on_short_or_missing_file` | R17 |
| `lsns_continue_after_reopen` | R11 |
| `truncation_at_every_byte_of_the_last_record_recovers_the_prefix` | R7, R8, R11 (every cut offset inside the last record, then append) |
| `read_all_stops_at_damage_and_leaves_the_file_alone` | R16 |
| `payload_bit_flip_discards_the_record_and_everything_after_it` | R7, R8, R9 |
| `length_field_larger_than_the_file_truncates_there` | R8, R10 |
| `sync_each_append_counts_one_sync_per_append` | R13, R15 |
| `without_sync_each_append_only_explicit_sync_counts` | R13, R14, R15 |
| `max_record_len_is_enforced_on_append` | R6, R15 |
| `record_longer_than_max_record_len_is_invalid_on_open` | R12 |

## Acceptance
```text
cd crates/ex05-write-ahead-log
cargo test                                  # exit code 0, "13 passed; 0 failed"
cargo test --release                        # same
cargo clippy --all-targets -- -D warnings   # exit code 0
cargo fmt --check                           # exit code 0
```

## Extra tests to write
1. Torn write inside the record prefix: truncate at 1, 2, 3, 5, 7 bytes into the 8-byte prefix of the last record. Catches reading a partial length as a short record.
2. Corrupt a byte in the file header of a file with records, then check `open` fails with `InvalidData` and the file bytes are unchanged. Catches truncating before validating the header.
3. Flip each single bit of one record (all `8 + len` bytes x 8 bits, in a loop). Expected: that record and every later one are dropped, earlier ones kept, `open` never panics. Catches panics on huge length values and a missing CRC check.
4. Zero-filled tail: append 4,096 zero bytes to a valid file. Eight zero bytes are a valid empty record, because the CRC of an empty payload is 0, so recovery keeps 512 phantom records. Assert that behavior, then explain in a comment why a preallocated file needs a different record marker.
5. An oversized `append` in the middle of a run must not change `sync_count` or the next LSN.
6. Generate payloads and a cut offset with a small seeded PRNG (or `proptest`), then compare `records_recovered` with a model that computes the expected prefix.

## Stretch (optional)
1. Share the log between threads: make `append` take `&self` using a `Mutex` around the file, and test with several threads that LSNs are gap-free.
2. Segment rotation: roll to a new file at 64 MiB, name files by first LSN, and add `read_from(lsn)`. Decide what a corrupt record in an older segment means.
3. Extend the CRC to cover `payload_len`. This detects a corrupted length that still points at bytes that happen to checksum correctly. Bump the header to `WALv0002` and reject `WALv0001`.
