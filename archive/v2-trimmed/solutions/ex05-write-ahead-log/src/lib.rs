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

use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, Read, Seek, SeekFrom, Write};
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
pub struct Wal {
    file: File,
    opts: WalOptions,
    /// File offset just past the last valid record.
    end: u64,
    next_lsn: u64,
    sync_count: u64,
}

/// Walks the records of `file` (length `len`) starting after the file header.
/// Calls `visit` with each valid payload. Returns `(records, end_of_last_valid)`.
fn scan(
    file: &File,
    len: u64,
    max_record_len: u32,
    mut visit: impl FnMut(Vec<u8>),
) -> io::Result<(u64, u64)> {
    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(FILE_HEADER.len() as u64))?;
    let mut pos = FILE_HEADER.len() as u64;
    let mut count = 0;
    loop {
        let mut head = [0u8; RECORD_HEADER_LEN as usize];
        if !read_full(&mut reader, &mut head)? {
            break;
        }
        let payload_len = u32::from_le_bytes(head[..4].try_into().unwrap());
        let crc = u32::from_le_bytes(head[4..].try_into().unwrap());
        // Compare against the file size before allocating: a corrupt length
        // must not turn into a multi-gigabyte buffer.
        if payload_len > max_record_len || u64::from(payload_len) > len - pos - RECORD_HEADER_LEN {
            break;
        }
        let mut payload = vec![0u8; payload_len as usize];
        if !read_full(&mut reader, &mut payload)? || crc32fast::hash(&payload) != crc {
            break;
        }
        pos += RECORD_HEADER_LEN + u64::from(payload_len);
        count += 1;
        visit(payload);
    }
    Ok((count, pos))
}

/// Fills `buf`. `Ok(false)` means EOF came first; other errors propagate.
fn read_full(r: &mut impl Read, buf: &mut [u8]) -> io::Result<bool> {
    match r.read_exact(buf) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(false),
        Err(e) => Err(e),
    }
}

fn check_header(file: &File) -> io::Result<()> {
    let mut header = [0u8; 8];
    let mut r = file;
    r.seek(SeekFrom::Start(0))?;
    r.read_exact(&mut header)?;
    if &header != FILE_HEADER {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "bad WAL file header",
        ));
    }
    Ok(())
}

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
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        let len = file.metadata()?.len();

        let (recovery, end) = if len < FILE_HEADER.len() as u64 {
            file.set_len(0)?;
            file.seek(SeekFrom::Start(0))?;
            file.write_all(FILE_HEADER)?;
            file.sync_data()?;
            let r = Recovery {
                records_recovered: 0,
                bytes_truncated: 0,
            };
            (r, FILE_HEADER.len() as u64)
        } else {
            check_header(&file)?;
            let (count, valid_end) = scan(&file, len, opts.max_record_len, drop)?;
            if valid_end < len {
                file.set_len(valid_end)?;
                file.sync_data()?;
            }
            let r = Recovery {
                records_recovered: count,
                bytes_truncated: len - valid_end,
            };
            (r, valid_end)
        };
        file.seek(SeekFrom::Start(end))?;

        let wal = Wal {
            file,
            opts,
            end,
            next_lsn: recovery.records_recovered,
            sync_count: 0,
        };
        Ok((wal, recovery))
    }

    /// Appends one record and returns its 0-based LSN.
    ///
    /// - `payload.len() > max_record_len` -> `ErrorKind::InvalidInput`; nothing is
    ///   written and no LSN is consumed.
    /// - The record reaches the file in a single `write_all` call.
    /// - With `sync_each_append`, calls `sync_data` before returning and counts it.
    pub fn append(&mut self, payload: &[u8]) -> io::Result<u64> {
        if payload.len() > self.opts.max_record_len as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "payload exceeds max_record_len",
            ));
        }
        let mut buf = Vec::with_capacity(RECORD_HEADER_LEN as usize + payload.len());
        buf.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        buf.extend_from_slice(&crc32fast::hash(payload).to_le_bytes());
        buf.extend_from_slice(payload);

        if let Err(e) = self.file.write_all(&buf) {
            // A partial record left behind is cut off by the next `open`.
            let _ = self.file.set_len(self.end);
            let _ = self.file.seek(SeekFrom::Start(self.end));
            return Err(e);
        }
        self.end += buf.len() as u64;
        let lsn = self.next_lsn;
        self.next_lsn += 1;
        if self.opts.sync_each_append {
            self.sync()?;
        }
        Ok(lsn)
    }

    /// Calls `sync_data` now and increments `sync_count`.
    pub fn sync(&mut self) -> io::Result<()> {
        self.file.sync_data()?;
        self.sync_count += 1;
        Ok(())
    }

    /// Number of `sync_data` calls made by `append` and `sync` on this handle.
    /// Calls made inside `open` are not counted.
    pub fn sync_count(&self) -> u64 {
        self.sync_count
    }

    /// Returns the payloads of the valid records in file order without
    /// modifying the file. Stops at the first invalid or incomplete record.
    ///
    /// - Missing file -> the `NotFound` error from the OS.
    /// - Shorter than 8 bytes -> `Ok(vec![])`.
    /// - 8 or more bytes with a wrong header -> `ErrorKind::InvalidData`.
    pub fn read_all(path: &Path, max_record_len: u32) -> io::Result<Vec<Vec<u8>>> {
        let file = File::open(path)?;
        let len = file.metadata()?.len();
        if len < FILE_HEADER.len() as u64 {
            return Ok(Vec::new());
        }
        check_header(&file)?;
        let mut out = Vec::new();
        scan(&file, len, max_record_len, |p| out.push(p))?;
        Ok(out)
    }
}
