//! Append-only write-ahead log with crash recovery.
//!
//! You design the file format. The tests only use the three items below and
//! look at the file as opaque bytes.

use std::io;
use std::path::Path;

/// What `Wal::open` found in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recovery {
    /// Number of records the log holds after `open` returns.
    pub records_recovered: u64,
}

/// Handle to an open log.
// Add the fields you need.
pub struct Wal {}

impl Wal {
    /// Opens the log at `path`, creating it if the file does not exist, and
    /// repairs damage left by a crash. Records larger than `max_record_len`
    /// bytes are invalid. See the exercise text for the full contract.
    pub fn open(path: &Path, max_record_len: u32) -> io::Result<(Wal, Recovery)> {
        let _ = (path, max_record_len);
        todo!()
    }

    /// Appends one record and returns its 0-based sequence number.
    /// A payload longer than `max_record_len` fails with
    /// `ErrorKind::InvalidInput` and consumes no sequence number.
    pub fn append(&mut self, payload: &[u8]) -> io::Result<u64> {
        let _ = payload;
        todo!()
    }

    /// Returns the payloads of the intact records in order, without
    /// modifying the file.
    pub fn read_all(path: &Path, max_record_len: u32) -> io::Result<Vec<Vec<u8>>> {
        let _ = (path, max_record_len);
        todo!()
    }
}
