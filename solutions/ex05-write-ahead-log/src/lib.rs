//! Append-only write-ahead log.
//!
//! File: records of `len: u32 LE | crc: u32 LE | payload`. The CRC-32 covers the length bytes
//! and the payload.

use std::fs::{File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::Path;

const HEADER: usize = 8;

/// Handle to an open log.
pub struct Wal {
    file: File,
}

fn crc(len_bytes: &[u8], payload: &[u8]) -> u32 {
    let mut h = crc32fast::Hasher::new();
    h.update(len_bytes);
    h.update(payload);
    h.finalize()
}

/// Returns the valid records at the start of `bytes` and the offset where they end.
fn scan(bytes: &[u8]) -> (Vec<Vec<u8>>, usize) {
    let mut records = Vec::new();
    let mut pos = 0;
    while let Some(header) = bytes.get(pos..pos + HEADER) {
        let len_bytes = &header[..4];
        let len = u32::from_le_bytes(len_bytes.try_into().unwrap()) as usize;
        let stored = u32::from_le_bytes(header[4..].try_into().unwrap());
        // A damaged length can name more bytes than the file holds.
        let Some(payload) = bytes.get(pos + HEADER..pos + HEADER + len) else {
            break;
        };
        if crc(len_bytes, payload) != stored {
            break;
        }
        records.push(payload.to_vec());
        pos += HEADER + len;
    }
    (records, pos)
}

impl Wal {
    /// Opens the log at `path`, creating the file if it is missing, and returns the log with
    /// the records recovered from the file, in order.
    pub fn open(path: &Path) -> io::Result<(Wal, Vec<Vec<u8>>)> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        let bytes = std::fs::read(path)?;
        let (records, end) = scan(&bytes);
        if end < bytes.len() {
            file.set_len(end as u64)?;
            file.sync_data()?;
        }
        file.seek(SeekFrom::Start(end as u64))?;
        Ok((Wal { file }, records))
    }

    /// Adds one record. When this returns `Ok`, the record is in the file.
    pub fn append(&mut self, record: &[u8]) -> io::Result<()> {
        let len_bytes = (record.len() as u32).to_le_bytes();
        let mut buf = Vec::with_capacity(HEADER + record.len());
        buf.extend_from_slice(&len_bytes);
        buf.extend_from_slice(&crc(&len_bytes, record).to_le_bytes());
        buf.extend_from_slice(record);
        self.file.write_all(&buf)
    }
}
