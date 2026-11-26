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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalOptions {
    /// When true, `append` calls `File::sync_data` before it returns.
    pub sync_each_append: bool,
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

/// Handle to an open log.
// Add the fields you need, for example: the `File`, the options, the offset
// just past the last valid record, the next LSN and the sync counter.
pub struct Wal {}

impl Wal {
    /// Opens or creates the log at `path` and repairs it.
    ///
    /// - Missing, empty or shorter than 8 bytes: (re)write the header, sync it,
    ///   return `Recovery { records_recovered: 0, bytes_truncated: 0 }`.
    /// - 8 or more bytes with a header other than `FILE_HEADER` -> `ErrorKind::InvalidData`,
    ///   file untouched.
    /// - Otherwise scan records from offset 8. At the first invalid or incomplete
    ///   record, `set_len` the file to the end of the last valid record and sync it.
    ///   Every byte after that point is discarded, including later records that
    ///   are intact.
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
    /// - With `sync_each_append`, calls `sync_data` before returning and counts it.
    pub fn append(&mut self, payload: &[u8]) -> io::Result<u64> {
        let _ = payload;
        todo!()
    }

    /// Calls `sync_data` now and increments `sync_count`.
    pub fn sync(&mut self) -> io::Result<()> {
        todo!()
    }

    /// Number of `sync_data` calls made by `append` and `sync` on this handle.
    /// Calls made inside `open` are not counted.
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
