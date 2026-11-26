//! Append-only write-ahead log.
//!
//! Replace each `todo!()`. Keep the public signatures. You choose the file format.

use std::io;
use std::path::Path;

/// Handle to an open log.
pub struct Wal {
    // Replace this field with your own.
}

impl Wal {
    /// Opens the log at `path`, creating the file if it is missing, and returns the log with
    /// the records recovered from the file, in order.
    pub fn open(path: &Path) -> io::Result<(Wal, Vec<Vec<u8>>)> {
        let _ = path;
        todo!()
    }

    /// Adds one record. When this returns `Ok`, the record is in the file.
    pub fn append(&mut self, record: &[u8]) -> io::Result<()> {
        let _ = record;
        todo!()
    }
}
