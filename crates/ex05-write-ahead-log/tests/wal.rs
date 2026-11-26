use ex05_write_ahead_log::{FILE_HEADER, RECORD_HEADER_LEN, Recovery, Wal, WalOptions};
use std::fs::{self, OpenOptions};
use std::io::ErrorKind;
use std::path::Path;

const MAX: u32 = 1 << 20;

fn opts() -> WalOptions {
    WalOptions {
        sync_each_append: false,
        max_record_len: MAX,
    }
}

fn payload(i: usize) -> Vec<u8> {
    format!("record-{i}-{}", "x".repeat(i % 37)).into_bytes()
}

/// Byte offset just past each record, given the payloads.
fn record_ends(payloads: &[Vec<u8>]) -> Vec<u64> {
    let mut pos = FILE_HEADER.len() as u64;
    payloads
        .iter()
        .map(|p| {
            pos += RECORD_HEADER_LEN + p.len() as u64;
            pos
        })
        .collect()
}

fn write_log(path: &Path, payloads: &[Vec<u8>]) {
    let (mut wal, rec) = Wal::open(path, opts()).unwrap();
    assert_eq!(rec.records_recovered, 0);
    for p in payloads {
        wal.append(p).unwrap();
    }
}

fn flip_bit(path: &Path, offset: u64, bit: u8) {
    let mut bytes = fs::read(path).unwrap();
    bytes[offset as usize] ^= 1 << bit;
    fs::write(path, bytes).unwrap();
}

fn file_len(path: &Path) -> u64 {
    fs::metadata(path).unwrap().len()
}

fn truncate(path: &Path, len: u64) {
    OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap()
        .set_len(len)
        .unwrap();
}

// ---- Milestone 1: header + open ----

#[test]
fn header_handling() {
    let dir = tempfile::tempdir().unwrap();

    // Wrong header, 8 or more bytes: InvalidData, file untouched.
    let bad = dir.path().join("bad");
    fs::write(&bad, b"WALv0002 and some more bytes").unwrap();
    let err = Wal::open(&bad, opts()).err().unwrap();
    assert_eq!(err.kind(), ErrorKind::InvalidData);
    assert_eq!(fs::read(&bad).unwrap(), b"WALv0002 and some more bytes");
    let err = Wal::read_all(&bad, MAX).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidData);

    // Exactly 8 wrong bytes is still a wrong header.
    let eight = dir.path().join("eight");
    fs::write(&eight, b"XXXXXXXX").unwrap();
    assert_eq!(
        Wal::open(&eight, opts()).err().unwrap().kind(),
        ErrorKind::InvalidData
    );

    // Missing, empty and short files get a fresh header.
    for (name, initial) in [
        ("missing", None),
        ("empty", Some(&b""[..])),
        ("short", Some(&b"WAL"[..])),
        ("short7", Some(&b"garbage"[..])),
    ] {
        let path = dir.path().join(name);
        if let Some(bytes) = initial {
            fs::write(&path, bytes).unwrap();
        }
        let (mut wal, rec) = Wal::open(&path, opts()).unwrap();
        assert_eq!(
            rec,
            Recovery {
                records_recovered: 0,
                bytes_truncated: 0
            },
            "{name}"
        );
        assert_eq!(fs::read(&path).unwrap(), FILE_HEADER, "{name}");
        assert_eq!(wal.append(b"ok").unwrap(), 0);
    }
}

// ---- Milestone 2: append + read_all ----

#[test]
fn on_disk_format_is_little_endian_len_crc_payload() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("log");
    write_log(&path, &[b"hello".to_vec()]);
    let bytes = fs::read(&path).unwrap();
    assert_eq!(&bytes[..8], b"WALv0001");
    assert_eq!(&bytes[8..12], &5u32.to_le_bytes());
    assert_eq!(&bytes[12..16], &0x3610_a686u32.to_le_bytes()); // CRC-32 of "hello"
    assert_eq!(&bytes[16..], b"hello");
}

#[test]
fn append_returns_lsns_and_read_all_returns_payloads_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("log");
    let (mut wal, _) = Wal::open(&path, opts()).unwrap();
    assert_eq!(wal.append(b"").unwrap(), 0);
    assert_eq!(wal.append(b"a").unwrap(), 1);
    assert_eq!(wal.append(b"").unwrap(), 2);
    drop(wal);
    // An empty payload is a valid 8-byte record.
    assert_eq!(file_len(&path), 8 + 8 + (8 + 1) + 8);
    assert_eq!(
        Wal::read_all(&path, MAX).unwrap(),
        vec![b"".to_vec(), b"a".to_vec(), b"".to_vec()]
    );
}

#[test]
fn read_all_on_short_or_missing_file() {
    let dir = tempfile::tempdir().unwrap();
    let short = dir.path().join("short");
    fs::write(&short, b"WAL").unwrap();
    assert_eq!(Wal::read_all(&short, MAX).unwrap(), Vec::<Vec<u8>>::new());
    let err = Wal::read_all(&dir.path().join("nope"), MAX).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::NotFound);
}

// ---- Milestone 3: reopen and continue LSNs ----

#[test]
fn lsns_continue_after_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("log");
    let payloads: Vec<Vec<u8>> = (0..100).map(payload).collect();
    write_log(&path, &payloads);

    let (mut wal, rec) = Wal::open(&path, opts()).unwrap();
    assert_eq!(
        rec,
        Recovery {
            records_recovered: 100,
            bytes_truncated: 0
        }
    );
    assert_eq!(wal.append(b"next").unwrap(), 100);
    drop(wal);

    let all = Wal::read_all(&path, MAX).unwrap();
    assert_eq!(all.len(), 101);
    assert_eq!(&all[..100], &payloads[..]);
    assert_eq!(all[100], b"next");
}

// ---- Milestone 4: truncated tail ----

#[test]
fn truncation_at_every_byte_of_the_last_record_recovers_the_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let reference = dir.path().join("reference");
    let payloads: Vec<Vec<u8>> = (0..10).map(|i| payload(i * 3)).collect();
    write_log(&reference, &payloads);
    let ends = record_ends(&payloads);
    let last_start = ends[8];
    let last_end = ends[9];
    assert_eq!(file_len(&reference), last_end);

    for cut in last_start..last_end {
        let path = dir.path().join("cut");
        fs::copy(&reference, &path).unwrap();
        truncate(&path, cut);

        let (mut wal, rec) = Wal::open(&path, opts()).unwrap();
        assert_eq!(rec.records_recovered, 9, "cut at {cut}");
        assert_eq!(rec.bytes_truncated, cut - last_start, "cut at {cut}");
        assert_eq!(file_len(&path), last_start, "cut at {cut}");

        // Appending after recovery continues at LSN 9 and is readable.
        assert_eq!(wal.append(b"after-recovery").unwrap(), 9);
        drop(wal);
        let all = Wal::read_all(&path, MAX).unwrap();
        assert_eq!(all.len(), 10, "cut at {cut}");
        assert_eq!(&all[..9], &payloads[..9]);
        assert_eq!(all[9], b"after-recovery");
        let (_, rec) = Wal::open(&path, opts()).unwrap();
        assert_eq!(rec.bytes_truncated, 0, "cut at {cut}");
    }
}

#[test]
fn read_all_stops_at_damage_and_leaves_the_file_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("log");
    let payloads: Vec<Vec<u8>> = (0..10).map(payload).collect();
    write_log(&path, &payloads);
    let ends = record_ends(&payloads);
    truncate(&path, ends[9] - 2);
    let before = fs::read(&path).unwrap();
    let all = Wal::read_all(&path, MAX).unwrap();
    assert_eq!(all, payloads[..9].to_vec());
    assert_eq!(fs::read(&path).unwrap(), before);
}

// ---- Milestone 5: corruption ----

#[test]
fn payload_bit_flip_discards_the_record_and_everything_after_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("log");
    let payloads: Vec<Vec<u8>> = (0..10).map(payload).collect();
    write_log(&path, &payloads);
    let ends = record_ends(&payloads);
    let total = file_len(&path);
    // Record 5 is damaged; records 6..9 stay intact but must be dropped too.
    flip_bit(&path, ends[4] + RECORD_HEADER_LEN, 3);

    let (mut wal, rec) = Wal::open(&path, opts()).unwrap();
    assert_eq!(rec.records_recovered, 5);
    assert_eq!(rec.bytes_truncated, total - ends[4]);
    assert_eq!(file_len(&path), ends[4]);
    assert_eq!(wal.append(b"x").unwrap(), 5);
    drop(wal);
    let all = Wal::read_all(&path, MAX).unwrap();
    assert_eq!(all.len(), 6);
    assert_eq!(&all[..5], &payloads[..5]);
}

#[test]
fn length_field_larger_than_the_file_truncates_there() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("log");
    let payloads: Vec<Vec<u8>> = (0..20).map(payload).collect();
    write_log(&path, &payloads);
    let ends = record_ends(&payloads);
    let total = file_len(&path);

    // Setting bit 31 of record 7's length makes it ~2 GiB. The limit is raised
    // so the file-size check, not max_record_len, is what rejects it.
    let big = WalOptions {
        sync_each_append: false,
        max_record_len: u32::MAX,
    };
    flip_bit(&path, ends[6] + 3, 7);
    let (_, rec) = Wal::open(&path, big).unwrap();
    assert_eq!(rec.records_recovered, 7);
    assert_eq!(rec.bytes_truncated, total - ends[6]);
    assert_eq!(file_len(&path), ends[6]);
}

// ---- Milestone 6: sync counting and size limit ----

#[test]
fn sync_each_append_counts_one_sync_per_append() {
    let dir = tempfile::tempdir().unwrap();
    let o = WalOptions {
        sync_each_append: true,
        max_record_len: MAX,
    };
    let (mut wal, _) = Wal::open(&dir.path().join("log"), o).unwrap();
    assert_eq!(wal.sync_count(), 0, "open must not count");
    for i in 0..25 {
        wal.append(b"x").unwrap();
        assert_eq!(wal.sync_count(), i + 1);
    }
}

#[test]
fn without_sync_each_append_only_explicit_sync_counts() {
    let dir = tempfile::tempdir().unwrap();
    let (mut wal, _) = Wal::open(&dir.path().join("log"), opts()).unwrap();
    for _ in 0..50 {
        wal.append(b"x").unwrap();
    }
    assert_eq!(wal.sync_count(), 0);
    wal.sync().unwrap();
    wal.sync().unwrap();
    assert_eq!(wal.sync_count(), 2);
}

#[test]
fn max_record_len_is_enforced_on_append() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("log");
    let o = WalOptions {
        sync_each_append: true,
        max_record_len: 16,
    };
    let (mut wal, _) = Wal::open(&path, o).unwrap();
    assert_eq!(wal.append(&[7u8; 16]).unwrap(), 0);
    let len_before = file_len(&path);
    let err = wal.append(&[7u8; 17]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidInput);
    assert_eq!(file_len(&path), len_before);
    // The rejected append consumed no LSN and no sync.
    assert_eq!(wal.sync_count(), 1);
    assert_eq!(wal.append(b"next").unwrap(), 1);
}

#[test]
fn record_longer_than_max_record_len_is_invalid_on_open() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("log");
    let payloads = vec![vec![1u8; 10], vec![2u8; 100], vec![3u8; 10]];
    write_log(&path, &payloads);
    let ends = record_ends(&payloads);
    let small = WalOptions {
        sync_each_append: false,
        max_record_len: 50,
    };
    let (_, rec) = Wal::open(&path, small).unwrap();
    assert_eq!(rec.records_recovered, 1);
    assert_eq!(rec.bytes_truncated, ends[2] - ends[0]);
}
