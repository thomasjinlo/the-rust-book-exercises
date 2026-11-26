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
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

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

struct Inner {
    file: File,
    /// File offset just past the last valid record.
    end: u64,
    next_lsn: u64,
    appends_since_sync: u32,
}

/// Handle to an open log. Must be `Send + Sync`: many threads append through `&Wal`.
pub struct Wal {
    inner: Mutex<Inner>,
    opts: WalOptions,
    sync_count: AtomicU64,
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
        if opts.sync == SyncPolicy::EveryN(0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "SyncPolicy::EveryN(0) is invalid",
            ));
        }
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
            inner: Mutex::new(Inner {
                file,
                end,
                next_lsn: recovery.records_recovered,
                appends_since_sync: 0,
            }),
            opts,
            sync_count: AtomicU64::new(0),
        };
        Ok((wal, recovery))
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

        let mut g = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if let Err(e) = g.file.write_all(&buf) {
            // A partial write would be misread as the start of the next
            // record. Restoring the position is best effort; recovery
            // handles a tail we fail to remove.
            let end = g.end;
            let _ = g.file.set_len(end);
            let _ = g.file.seek(SeekFrom::Start(end));
            return Err(e);
        }
        g.end += buf.len() as u64;
        let lsn = g.next_lsn;
        g.next_lsn += 1;

        let due = match self.opts.sync {
            SyncPolicy::Always => true,
            SyncPolicy::EveryN(n) => {
                g.appends_since_sync += 1;
                g.appends_since_sync >= n
            }
            SyncPolicy::Never => false,
        };
        if due {
            g.appends_since_sync = 0;
            g.file.sync_data()?;
            self.sync_count.fetch_add(1, Ordering::Relaxed);
        }
        Ok(lsn)
    }

    /// Calls `sync_data` now and increments `sync_count`.
    pub fn sync(&self) -> io::Result<()> {
        let g = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        g.file.sync_data()?;
        self.sync_count.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Number of `sync_data` calls made by `append` (per policy) and `sync`
    /// on this handle. Calls made inside `open` are not counted.
    pub fn sync_count(&self) -> u64 {
        self.sync_count.load(Ordering::Relaxed)
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
