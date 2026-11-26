# Exercise 12: Write-ahead log with crash recovery

**Time:** ~5 h | **Area:** systems programming / storage / durability | **Crate:** `crates/ex12-write-ahead-log`
**Builds on:** none (standalone)

## Read first
- Rust Book ch09, error handling: https://doc.rust-lang.org/book/ch09-00-error-handling.html
- Rust Book ch12, file I/O in a real program: https://doc.rust-lang.org/book/ch12-00-an-io-project.html
- Rust Book ch16-03, `Mutex`: https://doc.rust-lang.org/book/ch16-03-shared-state.html
- Rust Book ch11, testing: https://doc.rust-lang.org/book/ch11-00-testing.html
- `std::fs::File` (`sync_data`, `set_len`): https://doc.rust-lang.org/std/fs/struct.File.html
- `std::fs::OpenOptions`: https://doc.rust-lang.org/std/fs/struct.OpenOptions.html
- `std::io` (`Read`, `Write`, `Seek`, `SeekFrom`, `BufReader`): https://doc.rust-lang.org/std/io/index.html
- `crc32fast`: https://docs.rs/crc32fast/latest/crc32fast/
- Write-ahead logging: https://en.wikipedia.org/wiki/Write-ahead_logging
- `fsync(2)` / `fdatasync(2)` semantics: https://man7.org/linux/man-pages/man2/fsync.2.html

Read the `fsync(2)` page before you start. A successful `write` only means the kernel has the bytes. After a power loss, only bytes covered by a completed `fsync`/`fdatasync` are guaranteed to be on disk. A crash in the middle of a write can also leave a prefix of the data, or in rare cases damaged bytes (a torn write).

## What you are building
A write-ahead log (WAL) is an append-only file that a database or queue writes before it applies a change, so that it can replay the change after a crash. The file can end in half a record when the process dies mid-write. The log must detect that on open, cut the damage off, and carry on appending.

You build `Wal`: a checksummed, length-prefixed record log. Many threads append through a shared `&Wal`. `open` scans the file, truncates it to the last intact record, and reports what it did. A `SyncPolicy` trades durability for throughput.

## Starting point
- Edit only `crates/ex12-write-ahead-log/src/lib.rs`. The public items, their doc comments and the constants `FILE_HEADER` and `RECORD_HEADER_LEN` are the contract. Add private fields and helpers freely.
- Do not edit `crates/ex12-write-ahead-log/tests/wal.rs` or `Cargo.toml`. Add your own tests in a separate file such as `tests/extra.rs`.
- Dependencies are already listed: `crc32fast` (CRC-32), `tempfile` (dev only).
- Run: `cd crates/ex12-write-ahead-log && cargo test`.

On the unmodified starter, `cargo test` finishes in under a second with 20 failures and 1 pass (the `Send + Sync` check).

## Requirements

On-disk format, all integers little-endian:

```text
file   := "WALv0001" record*
record := u32 payload_len | u32 crc32(payload) | payload bytes
```

`crc32` is `crc32fast::hash(payload)`. An empty payload is a valid record of 8 bytes.

**Opening**

R1. `Wal::open(path, opts)` returns `io::Result<(Wal, Recovery)>`. It creates the file if missing.
R2. `SyncPolicy::EveryN(0)` makes `open` return `ErrorKind::InvalidInput` before it creates or touches the file.
R3. A file that is missing, empty, or shorter than 8 bytes gets the 8-byte header written and synced (any bytes already there are discarded). The result is `Recovery { records_recovered: 0, bytes_truncated: 0 }`.
R4. A file of 8 or more bytes whose first 8 bytes are not `WALv0001` makes `open` return `ErrorKind::InvalidData`. The file is not modified.

**Recovery**

R5. A record is valid when all `8 + payload_len` bytes are in the file, `payload_len <= max_record_len` and the CRC matches.
R6. `open` scans from offset 8. At the first invalid or incomplete record it calls `File::set_len` with the end offset of the last valid record, then `sync_data`. It sets `bytes_truncated` to the number of bytes removed and `records_recovered` to the number of records kept. A clean file has `bytes_truncated == 0` and no `set_len`.
R7. Records after the first invalid record are discarded even if they are intact. Reason: a log is a prefix of the append history. Keeping a later record would replay a change whose predecessor is lost, and the LSN-to-position mapping would have a hole.
R8. A length field that points past the end of the file must be rejected before allocating `payload_len` bytes. A length field of `u32::MAX` must not allocate 4 GiB.
R9. `max_record_len` also applies on recovery. Reopening with a smaller limit makes records longer than the new limit invalid (and so truncated, per R6 and R7).
R10. After `open` the next LSN equals `records_recovered`, and appends go to the end of the recovered file.

**Appending**

R11. `append(&self, payload) -> io::Result<u64>` returns the record's 0-based LSN. LSNs are gap-free and equal the record's position in the file, also when called from many threads.
R12. `payload.len() > max_record_len` returns `ErrorKind::InvalidInput`, writes nothing and consumes no LSN. A payload of exactly `max_record_len` bytes is accepted.
R13. Each record reaches the file through one `write_all` call on a buffer that holds the 8-byte record prefix and the payload.
R14. If `write_all` fails, `append` returns the error, tries to `set_len` the file back to the end of the last good record, and consumes no LSN.
R15. `Wal` is `Send + Sync`. `append` takes `&self`.

**Syncing**

R16. `SyncPolicy::Always`: `sync_data` after each successful append. `EveryN(n)`: `sync_data` after the n-th, 2n-th, 3n-th ... successful append of this handle. `Never`: no sync from `append`. The sync happens before `append` returns.
R17. `sync(&self)` calls `sync_data` once. It does not change the `EveryN` schedule (R16 counts appends, not time since the last sync).
R18. `sync_count()` returns the number of `sync_data` calls made through `append` and `sync` on this handle. Calls made inside `open` are not counted. A new handle starts at 0.

**Reading**

R19. `Wal::read_all(path, max_record_len) -> io::Result<Vec<Vec<u8>>>` returns the valid records in file order and stops at the first invalid or incomplete one. It never modifies the file.
R20. `read_all` on a missing file returns `ErrorKind::NotFound`. A file shorter than 8 bytes gives `Ok(vec![])`. A file of 8 or more bytes with a wrong header gives `ErrorKind::InvalidData`.

## Given tests
All in `tests/wal.rs`. 21 run by default, 1 is `#[ignore]`.

| Test | Requirement |
|---|---|
| `thousand_records_roundtrip_and_lsns_continue` | R1, R10, R11, R19 |
| `empty_payload_is_a_valid_record` | R5, R11 |
| `on_disk_format_is_little_endian_len_crc_payload` | format, R13 |
| `truncation_at_every_byte_of_the_last_record_recovers_the_prefix` | R5, R6, R10 (every cut offset inside the last record) |
| `payload_bit_flip_in_record_500_discards_it_and_everything_after` | R5, R6, R7 |
| `intact_records_after_a_corrupt_one_are_discarded` | R7 |
| `length_field_larger_than_the_file_truncates_there` | R6, R8 |
| `length_field_just_past_end_of_file_is_incomplete` | R5, R8 |
| `record_longer_than_max_record_len_is_invalid_on_open` | R9 |
| `header_handling` | R3, R4, R20 |
| `read_all_on_short_or_missing_file` | R20 |
| `read_all_stops_at_damage_and_leaves_the_file_alone` | R19 |
| `sync_policy_always_syncs_every_append` | R16, R18 |
| `sync_policy_every_n_syncs_on_every_nth_append` | R16 (95 appends, N=10 -> 9 syncs) |
| `sync_policy_never_and_explicit_sync` | R16, R17, R18 |
| `explicit_sync_does_not_shift_the_every_n_schedule` | R17 |
| `every_n_zero_is_rejected_before_touching_the_file` | R2 |
| `max_record_len_is_enforced_on_append` | R12 |
| `wal_can_be_shared_across_threads` | R15 |
| `four_threads_append_gap_free_lsns_in_file_order` | R11 (4 threads x 1,000 appends; runs under a 60 s watchdog) |
| `crash_simulation_any_file_prefix_recovers_a_record_prefix` | R3, R5, R6, R10 (200 seeded random prefixes of a 60-record file) |
| `throughput_always_vs_every_100` (ignored) | measurement only |

## Acceptance
```text
cd crates/ex12-write-ahead-log
cargo test                 # exit code 0, "21 passed; 0 failed; 1 ignored"
cargo test --release       # same
cargo clippy --all-targets -- -D warnings   # exit code 0
cargo fmt --check          # exit code 0
```

Throughput, record the numbers in your notes (there is no threshold, because `fsync` cost depends on the disk):
```text
cargo test --release -- --ignored --nocapture
```
It prints appends per second and sync count for `Always` and `EveryN(100)` with 2,000 appends of 128 bytes. The sync counts must be 2000 and 20. On the reference run on a Linux dev host, `Always` took 507 appends/s and `EveryN(100)` 46,103 appends/s. Your disk will differ. Expect `EveryN(100)` to be at least an order of magnitude faster on a disk with a real flush.

## Extra tests to write
1. Torn write in the middle of the record prefix: truncate at 1, 2, 3, 5, 7 bytes into the 8-byte prefix of the last record. Catches reading a partial length as a short record.
2. Corrupt a byte in the file header of a file with records, then check `open` fails with `InvalidData` and the file bytes are unchanged. Catches truncating before validating the header.
3. Flip each single bit of one record (all `8 + len` bytes x 8 bits, in a loop). Expected: that record and every later one are dropped, earlier ones kept, `open` never panics. Catches panics on huge length values and missing CRC checks.
4. Zero-filled tail: append 4,096 zero bytes to a valid file. Eight zero bytes are a valid empty record, because `crc32` of an empty payload is 0, so recovery keeps 512 phantom records. Assert that behaviour, then explain in a comment why a preallocated file needs a different record marker (for example a non-zero magic byte or a CRC that covers the length plus a constant).
5. Open the same file twice in sequence without dropping the first handle, appending through both. Decide what you want and write a test for it. Catches the assumption of a single writer; see the stretch on file locks.
6. Append 10,000 records of random length (seeded PRNG) from 8 threads with `EveryN(7)`, reopen, check count and per-thread order. Catches lost updates and order violations across the lock.
7. `append` returning `InvalidInput` for an oversized payload mid-stream must not change `sync_count` or the next LSN. Catches counters updated before validation.
8. Use `proptest` to generate a list of payloads and a cut offset, then check R5 to R10 against a model that computes the expected prefix. Add `proptest` to `[dev-dependencies]` yourself.

## Stretch (optional)
1. Group commit: with `Always`, concurrent appenders currently pay one `sync_data` each, serialised by the mutex. Let one thread (the leader) sync once for every record written by the threads waiting behind it, and have followers return when the sync that covers their record completes. Measure appends/s with 8 threads before and after. `sync_count` then falls below the append count, so keep the original behaviour behind a flag so the given tests still hold.
2. Segment rotation with an index: roll to a new file at 64 MiB, name files by first LSN (`00000000000000001000.wal`), and keep an in-memory map from LSN to (segment, offset) rebuilt during recovery. Add `read_from(lsn)`. Only the last segment can have a torn tail; decide what a corrupt record in an older segment means.
3. Extend the CRC to cover `payload_len` and the LSN. This detects a length field that was corrupted but still points at bytes that happen to checksum correctly, and a record that was copied to the wrong position. Bump the header to `WALv0002` and make `open` reject `WALv0001`.
