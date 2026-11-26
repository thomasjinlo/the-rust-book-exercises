//! Append-only write-ahead log with crash recovery.
//!
//! On-disk format (all integers little-endian):
//!
//! ```text
//! file   := "WALv0001" record*
//! record := u32 payload_len | u32 crc32(payload) | payload bytes
//! ```
//!
//! A record is valid when it is completely present, `payload_len <= max_record_len`
//! and the CRC-32 (as computed by `crc32fast::hash`) matches the payload.

use std::io;
use std::path::Path;

/// The 8 bytes every log file starts with.
pub const FILE_HEADER: &[u8; 8] = b"WALv0001";
/// Size of the per-record prefix: `payload_len` (4 bytes) + `crc32` (4 bytes).
pub const RECORD_HEADER_LEN: u64 = 8;

/// When the log calls `File::sync_data`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncPolicy {
    /// After every successful append.
    Always,
    /// After every n-th successful append of this handle (the n-th, 2n-th, ...).
    /// `EveryN(0)` makes `Wal::open` fail with `ErrorKind::InvalidInput`.
    EveryN(u32),
    /// Only when `Wal::sync` is called.
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalOptions {
    pub sync: SyncPolicy,
    /// Largest accepted payload in bytes. Applies to appends and to recovery:
    /// an on-disk record whose length field exceeds it is invalid.
    pub max_record_len: u32,
}

/// What `Wal::open` found and repaired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recovery {
    /// Number of valid records in the file after recovery.
    pub records_recovered: u64,
    /// Bytes removed from the end of the file. 0 for a clean file, a new
    /// file, or a file shorter than the 8-byte header that was re-initialised.
    pub bytes_truncated: u64,
}

/// Handle to an open log. Must be `Send + Sync`: many threads append through `&Wal`.
// Add the fields you need: the file, its end offset and the next LSN behind a
// `Mutex`, plus an `AtomicU64` for `sync_count`.
pub struct Wal {}

impl Wal {
    /// Opens or creates the log at `path` and repairs it.
    ///
    /// - `EveryN(0)` -> `ErrorKind::InvalidInput`, before touching the file.
    /// - Missing, empty or shorter than 8 bytes: (re)write the header, sync it,
    ///   return `Recovery { records_recovered: 0, bytes_truncated: 0 }`.
    /// - 8 or more bytes with a header other than `FILE_HEADER` -> `ErrorKind::InvalidData`,
    ///   file untouched.
    /// - Otherwise scan records from offset 8. At the first invalid or incomplete
    ///   record, `set_len` the file to the end of the last valid record and sync it.
    ///   Every byte after that point is discarded, including later records that
    ///   are intact: the log promises a prefix of what was appended, never a
    ///   sequence with holes.
    /// - The next LSN is `records_recovered`.
    /// - `sync_data` calls made here do not count towards `sync_count`.
    pub fn open(path: &Path, opts: WalOptions) -> io::Result<(Wal, Recovery)> {
        let _ = (path, opts);
        todo!()
    }

    /// Appends one record and returns its 0-based LSN.
    ///
    /// - `payload.len() > max_record_len` -> `ErrorKind::InvalidInput`; nothing is
    ///   written and no LSN is consumed.
    /// - The record reaches the file in a single `write_all` call.
    /// - LSNs are gap-free and match file order, also across threads.
    /// - If the write fails, the file is cut back to the end of the last good
    ///   record (best effort) and no LSN is consumed.
    /// - Sync per `SyncPolicy` while still holding the lock.
    pub fn append(&self, payload: &[u8]) -> io::Result<u64> {
        let _ = payload;
        todo!()
    }

    /// Calls `sync_data` now and increments `sync_count`.
    pub fn sync(&self) -> io::Result<()> {
        todo!()
    }

    /// Number of `sync_data` calls made by `append` (per policy) and `sync`
    /// on this handle. Calls made inside `open` are not counted.
    pub fn sync_count(&self) -> u64 {
        todo!()
    }

    /// Returns the payloads of the valid records in file order without
    /// modifying the file. Stops at the first invalid or incomplete record.
    ///
    /// - Missing file -> the `NotFound` error from the OS.
    /// - Shorter than 8 bytes -> `Ok(vec![])`.
    /// - 8 or more bytes with a wrong header -> `ErrorKind::InvalidData`.
    pub fn read_all(path: &Path, max_record_len: u32) -> io::Result<Vec<Vec<u8>>> {
        let _ = (path, max_record_len);
        todo!()
    }
}
