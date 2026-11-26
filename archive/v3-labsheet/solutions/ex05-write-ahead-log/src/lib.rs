//! Append-only write-ahead log with crash recovery.
//!
//! File: 8-byte magic, then records of `len: u32 LE | crc: u32 LE | payload`.
//! The CRC-32 covers the length bytes and the payload.

use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, Read, Seek, SeekFrom, Write};
use std::path::Path;

const MAGIC: &[u8; 8] = b"WALv0001";
const PREFIX_LEN: u64 = 8;

/// What `Wal::open` found in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recovery {
    /// Number of records the log holds after `open` returns.
    pub records_recovered: u64,
}

/// Handle to an open log.
pub struct Wal {
    file: File,
    max_record_len: u32,
    /// Offset just past the last intact record.
    end: u64,
    next_seq: u64,
}

fn record_crc(len_bytes: [u8; 4], payload: &[u8]) -> u32 {
    let mut h = crc32fast::Hasher::new();
    h.update(&len_bytes);
    h.update(payload);
    h.finalize()
}

/// Fills `buf`; `Ok(false)` means the file ended first.
fn read_full(r: &mut impl Read, buf: &mut [u8]) -> io::Result<bool> {
    match r.read_exact(buf) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(false),
        Err(e) => Err(e),
    }
}

/// Walks the intact records of a file of `len` bytes (magic already checked).
/// Returns the record count and the offset just past the last intact one.
fn scan(
    file: &File,
    len: u64,
    max_record_len: u32,
    mut visit: impl FnMut(Vec<u8>),
) -> io::Result<(u64, u64)> {
    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(MAGIC.len() as u64))?;
    let mut pos = MAGIC.len() as u64;
    let mut count = 0;
    loop {
        let mut prefix = [0u8; PREFIX_LEN as usize];
        if !read_full(&mut reader, &mut prefix)? {
            break;
        }
        let len_bytes: [u8; 4] = prefix[..4].try_into().unwrap();
        let payload_len = u32::from_le_bytes(len_bytes);
        let crc = u32::from_le_bytes(prefix[4..].try_into().unwrap());
        // Check against the bytes left in the file before allocating, so a
        // damaged length cannot request gigabytes.
        if payload_len > max_record_len || u64::from(payload_len) > len - pos - PREFIX_LEN {
            break;
        }
        let mut payload = vec![0u8; payload_len as usize];
        if !read_full(&mut reader, &mut payload)? || record_crc(len_bytes, &payload) != crc {
            break;
        }
        pos += PREFIX_LEN + u64::from(payload_len);
        count += 1;
        visit(payload);
    }
    Ok((count, pos))
}

/// `Ok(true)` when the file starts with a full magic, `Ok(false)` when it is a
/// proper prefix of the magic (a torn creation). Anything else is foreign.
fn check_magic(file: &File, len: u64) -> io::Result<bool> {
    let mut head = vec![0u8; len.min(MAGIC.len() as u64) as usize];
    let mut r = file;
    r.seek(SeekFrom::Start(0))?;
    r.read_exact(&mut head)?;
    if head.len() == MAGIC.len() && head == MAGIC {
        Ok(true)
    } else if head.len() < MAGIC.len() && MAGIC.starts_with(&head) {
        Ok(false)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not a write-ahead log file",
        ))
    }
}

impl Wal {
    /// Opens the log at `path`, creating it if the file does not exist, and
    /// cuts off anything after the last intact record.
    pub fn open(path: &Path, max_record_len: u32) -> io::Result<(Wal, Recovery)> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        let len = file.metadata()?.len();

        let (count, end) = if check_magic(&file, len)? {
            let (count, end) = scan(&file, len, max_record_len, drop)?;
            if end < len {
                file.set_len(end)?;
                file.sync_data()?;
            }
            (count, end)
        } else {
            file.set_len(0)?;
            file.seek(SeekFrom::Start(0))?;
            file.write_all(MAGIC)?;
            file.sync_data()?;
            (0, MAGIC.len() as u64)
        };
        // The scan leaves the cursor elsewhere; appends go to the end.
        file.seek(SeekFrom::Start(end))?;

        let wal = Wal {
            file,
            max_record_len,
            end,
            next_seq: count,
        };
        Ok((
            wal,
            Recovery {
                records_recovered: count,
            },
        ))
    }

    /// Appends one record and returns its 0-based sequence number.
    pub fn append(&mut self, payload: &[u8]) -> io::Result<u64> {
        if payload.len() > self.max_record_len as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "payload exceeds max_record_len",
            ));
        }
        let len_bytes = (payload.len() as u32).to_le_bytes();
        let mut buf = Vec::with_capacity(PREFIX_LEN as usize + payload.len());
        buf.extend_from_slice(&len_bytes);
        buf.extend_from_slice(&record_crc(len_bytes, payload).to_le_bytes());
        buf.extend_from_slice(payload);

        if let Err(e) = self.file.write_all(&buf) {
            // Remove a partial record so the next append starts clean.
            let _ = self.file.set_len(self.end);
            let _ = self.file.seek(SeekFrom::Start(self.end));
            return Err(e);
        }
        self.end += buf.len() as u64;
        self.next_seq += 1;
        Ok(self.next_seq - 1)
    }

    /// Returns the payloads of the intact records in order, without
    /// modifying the file.
    pub fn read_all(path: &Path, max_record_len: u32) -> io::Result<Vec<Vec<u8>>> {
        let file = File::open(path)?;
        let len = file.metadata()?.len();
        let mut out = Vec::new();
        if check_magic(&file, len)? {
            scan(&file, len, max_record_len, |p| out.push(p))?;
        }
        Ok(out)
    }
}
