use ex12_write_ahead_log::{FILE_HEADER, RECORD_HEADER_LEN, Recovery, SyncPolicy, Wal, WalOptions};
use std::fs::{self, OpenOptions};
use std::io::ErrorKind;
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const MAX: u32 = 1 << 20;

fn opts(sync: SyncPolicy) -> WalOptions {
    WalOptions {
        sync,
        max_record_len: MAX,
    }
}

/// Runs `body` on its own thread and fails the test if it does not finish.
/// 60 s is far above the ~1 s these bodies need; it only exists so a
/// deadlocked implementation fails instead of hanging `cargo test`.
fn with_watchdog<F: FnOnce() + Send + 'static>(body: F) {
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        body();
        let _ = tx.send(());
    });
    match rx.recv_timeout(Duration::from_secs(60)) {
        Ok(()) => handle.join().unwrap(),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            handle.join().unwrap();
            unreachable!("body panicked but join did not")
        }
        Err(mpsc::RecvTimeoutError::Timeout) => panic!("test body did not finish in 60 s"),
    }
}

fn payload(i: usize) -> Vec<u8> {
    format!("record-{i}-{}", "x".repeat(i % 37)).into_bytes()
}

/// Byte offset just past each record, given the payload sizes.
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
    let (wal, rec) = Wal::open(path, opts(SyncPolicy::Never)).unwrap();
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

#[test]
fn thousand_records_roundtrip_and_lsns_continue() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("log");
    {
        let (wal, rec) = Wal::open(&path, opts(SyncPolicy::Never)).unwrap();
        assert_eq!(
            rec,
            Recovery {
                records_recovered: 0,
                bytes_truncated: 0
            }
        );
        for i in 0..1000 {
            assert_eq!(wal.append(&payload(i)).unwrap(), i as u64);
        }
    }
    let (wal, rec) = Wal::open(&path, opts(SyncPolicy::Never)).unwrap();
    assert_eq!(
        rec,
        Recovery {
            records_recovered: 1000,
            bytes_truncated: 0
        }
    );
    assert_eq!(wal.append(b"next").unwrap(), 1000);
    drop(wal);

    let all = Wal::read_all(&path, MAX).unwrap();
    assert_eq!(all.len(), 1001);
    for (i, p) in all.iter().take(1000).enumerate() {
        assert_eq!(p, &payload(i));
    }
    assert_eq!(all[1000], b"next");
}

#[test]
fn empty_payload_is_a_valid_record() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("log");
    {
        let (wal, _) = Wal::open(&path, opts(SyncPolicy::Never)).unwrap();
        assert_eq!(wal.append(b"").unwrap(), 0);
        assert_eq!(wal.append(b"a").unwrap(), 1);
        assert_eq!(wal.append(b"").unwrap(), 2);
    }
    assert_eq!(file_len(&path), 8 + 8 + (8 + 1) + 8);
    assert_eq!(
        Wal::read_all(&path, MAX).unwrap(),
        vec![b"".to_vec(), b"a".to_vec(), b"".to_vec()]
    );
    let (_, rec) = Wal::open(&path, opts(SyncPolicy::Never)).unwrap();
    assert_eq!(rec.records_recovered, 3);
    assert_eq!(rec.bytes_truncated, 0);
}

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
        OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(cut)
            .unwrap();

        let (wal, rec) = Wal::open(&path, opts(SyncPolicy::Never)).unwrap();
        assert_eq!(rec.records_recovered, 9, "cut at {cut}");
        assert_eq!(rec.bytes_truncated, cut - last_start, "cut at {cut}");
        assert_eq!(file_len(&path), last_start, "cut at {cut}");

        assert_eq!(wal.append(b"after-recovery").unwrap(), 9);
        drop(wal);
        let all = Wal::read_all(&path, MAX).unwrap();
        assert_eq!(all.len(), 10, "cut at {cut}");
        assert_eq!(&all[..9], &payloads[..9]);
        assert_eq!(all[9], b"after-recovery");
        let (_, rec) = Wal::open(&path, opts(SyncPolicy::Never)).unwrap();
        assert_eq!(rec.bytes_truncated, 0, "cut at {cut}");
    }
}

#[test]
fn payload_bit_flip_in_record_500_discards_it_and_everything_after() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("log");
    let payloads: Vec<Vec<u8>> = (0..1000).map(payload).collect();
    write_log(&path, &payloads);
    let ends = record_ends(&payloads);
    let total = file_len(&path);
    let start_500 = ends[499];
    // First payload byte of record 500.
    flip_bit(&path, start_500 + RECORD_HEADER_LEN, 3);

    let (wal, rec) = Wal::open(&path, opts(SyncPolicy::Never)).unwrap();
    assert_eq!(rec.records_recovered, 500);
    assert_eq!(rec.bytes_truncated, total - start_500);
    assert_eq!(file_len(&path), start_500);
    assert_eq!(wal.append(b"x").unwrap(), 500);
    drop(wal);
    let all = Wal::read_all(&path, MAX).unwrap();
    assert_eq!(all.len(), 501);
    assert_eq!(&all[..500], &payloads[..500]);
}

#[test]
fn intact_records_after_a_corrupt_one_are_discarded() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("log");
    let payloads: Vec<Vec<u8>> = (0..10).map(|i| vec![i as u8; 20]).collect();
    write_log(&path, &payloads);
    let ends = record_ends(&payloads);
    // Corrupt the CRC field of record 3; records 4..10 stay intact.
    flip_bit(&path, ends[2] + 4, 0);
    let (_, rec) = Wal::open(&path, opts(SyncPolicy::Never)).unwrap();
    assert_eq!(rec.records_recovered, 3);
    assert_eq!(rec.bytes_truncated, ends[9] - ends[2]);
    assert_eq!(file_len(&path), ends[2]);
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
        sync: SyncPolicy::Never,
        max_record_len: u32::MAX,
    };
    flip_bit(&path, ends[6] + 3, 7);
    let (_, rec) = Wal::open(&path, big).unwrap();
    assert_eq!(rec.records_recovered, 7);
    assert_eq!(rec.bytes_truncated, total - ends[6]);
    assert_eq!(file_len(&path), ends[6]);
}

#[test]
fn length_field_just_past_end_of_file_is_incomplete() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("log");
    let payloads: Vec<Vec<u8>> = vec![b"aaaa".to_vec(), b"bbbb".to_vec()];
    write_log(&path, &payloads);
    let ends = record_ends(&payloads);
    // Record 1 claims 5 bytes but only 4 exist.
    let mut bytes = fs::read(&path).unwrap();
    bytes[ends[0] as usize..ends[0] as usize + 4].copy_from_slice(&5u32.to_le_bytes());
    fs::write(&path, bytes).unwrap();
    let (_, rec) = Wal::open(&path, opts(SyncPolicy::Never)).unwrap();
    assert_eq!(rec.records_recovered, 1);
    assert_eq!(rec.bytes_truncated, ends[1] - ends[0]);
}

#[test]
fn record_longer_than_max_record_len_is_invalid_on_open() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("log");
    let payloads = vec![vec![1u8; 10], vec![2u8; 100], vec![3u8; 10]];
    write_log(&path, &payloads);
    let ends = record_ends(&payloads);
    let small = WalOptions {
        sync: SyncPolicy::Never,
        max_record_len: 50,
    };
    let (_, rec) = Wal::open(&path, small).unwrap();
    assert_eq!(rec.records_recovered, 1);
    assert_eq!(rec.bytes_truncated, ends[2] - ends[0]);
}

#[test]
fn header_handling() {
    let dir = tempfile::tempdir().unwrap();

    // Wrong header, 8 or more bytes: InvalidData, file untouched.
    let bad = dir.path().join("bad");
    fs::write(&bad, b"WALv0002 and some more bytes").unwrap();
    let err = Wal::open(&bad, opts(SyncPolicy::Never)).err().unwrap();
    assert_eq!(err.kind(), ErrorKind::InvalidData);
    assert_eq!(fs::read(&bad).unwrap(), b"WALv0002 and some more bytes");
    let err = Wal::read_all(&bad, MAX).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidData);

    // Exactly 8 wrong bytes is still a wrong header.
    let eight = dir.path().join("eight");
    fs::write(&eight, b"XXXXXXXX").unwrap();
    assert_eq!(
        Wal::open(&eight, opts(SyncPolicy::Never))
            .err()
            .unwrap()
            .kind(),
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
        let (wal, rec) = Wal::open(&path, opts(SyncPolicy::Never)).unwrap();
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

#[test]
fn read_all_on_short_or_missing_file() {
    let dir = tempfile::tempdir().unwrap();
    let short = dir.path().join("short");
    fs::write(&short, b"WAL").unwrap();
    assert_eq!(Wal::read_all(&short, MAX).unwrap(), Vec::<Vec<u8>>::new());
    let err = Wal::read_all(&dir.path().join("nope"), MAX).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::NotFound);
}

#[test]
fn read_all_stops_at_damage_and_leaves_the_file_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("log");
    let payloads: Vec<Vec<u8>> = (0..10).map(payload).collect();
    write_log(&path, &payloads);
    let ends = record_ends(&payloads);
    let cut = ends[9] - 2;
    OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(cut)
        .unwrap();
    let before = fs::read(&path).unwrap();
    let all = Wal::read_all(&path, MAX).unwrap();
    assert_eq!(all, payloads[..9].to_vec());
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn sync_policy_always_syncs_every_append() {
    let dir = tempfile::tempdir().unwrap();
    let (wal, _) = Wal::open(&dir.path().join("log"), opts(SyncPolicy::Always)).unwrap();
    assert_eq!(wal.sync_count(), 0, "open must not count");
    for i in 0..25 {
        wal.append(b"x").unwrap();
        assert_eq!(wal.sync_count(), i + 1);
    }
}

#[test]
fn sync_policy_every_n_syncs_on_every_nth_append() {
    let dir = tempfile::tempdir().unwrap();
    let (wal, _) = Wal::open(&dir.path().join("log"), opts(SyncPolicy::EveryN(10))).unwrap();
    for i in 1..=95u64 {
        wal.append(b"x").unwrap();
        assert_eq!(wal.sync_count(), i / 10, "after append {i}");
    }
    assert_eq!(wal.sync_count(), 9);
}

#[test]
fn sync_policy_never_and_explicit_sync() {
    let dir = tempfile::tempdir().unwrap();
    let (wal, _) = Wal::open(&dir.path().join("log"), opts(SyncPolicy::Never)).unwrap();
    for _ in 0..50 {
        wal.append(b"x").unwrap();
    }
    assert_eq!(wal.sync_count(), 0);
    wal.sync().unwrap();
    wal.sync().unwrap();
    assert_eq!(wal.sync_count(), 2);
}

#[test]
fn explicit_sync_does_not_shift_the_every_n_schedule() {
    let dir = tempfile::tempdir().unwrap();
    let (wal, _) = Wal::open(&dir.path().join("log"), opts(SyncPolicy::EveryN(4))).unwrap();
    for _ in 0..3 {
        wal.append(b"x").unwrap();
    }
    wal.sync().unwrap();
    assert_eq!(wal.sync_count(), 1);
    wal.append(b"x").unwrap(); // 4th append: policy sync
    assert_eq!(wal.sync_count(), 2);
}

#[test]
fn every_n_zero_is_rejected_before_touching_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("log");
    let err = Wal::open(&path, opts(SyncPolicy::EveryN(0))).err().unwrap();
    assert_eq!(err.kind(), ErrorKind::InvalidInput);
    assert!(!path.exists());
}

#[test]
fn max_record_len_is_enforced_on_append() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("log");
    let o = WalOptions {
        sync: SyncPolicy::Never,
        max_record_len: 16,
    };
    let (wal, _) = Wal::open(&path, o).unwrap();
    assert_eq!(wal.append(&[7u8; 16]).unwrap(), 0);
    let len_before = file_len(&path);
    let err = wal.append(&[7u8; 17]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidInput);
    assert_eq!(file_len(&path), len_before);
    // The rejected append consumed no LSN.
    assert_eq!(wal.append(b"next").unwrap(), 1);
}

#[test]
fn wal_can_be_shared_across_threads() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Wal>();
}

#[test]
fn four_threads_append_gap_free_lsns_in_file_order() {
    with_watchdog(|| {
        const THREADS: usize = 4;
        const PER_THREAD: usize = 1000;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log");
        let (wal, _) = Wal::open(&path, opts(SyncPolicy::Never)).unwrap();

        let results: Vec<Vec<(u64, Vec<u8>)>> = thread::scope(|s| {
            let handles: Vec<_> = (0..THREADS)
                .map(|t| {
                    let wal = &wal;
                    s.spawn(move || {
                        (0..PER_THREAD)
                            .map(|i| {
                                let p = format!("{t}:{i}").into_bytes();
                                (wal.append(&p).unwrap(), p)
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        drop(wal);

        let mut lsns: Vec<u64> = results.iter().flatten().map(|(l, _)| *l).collect();
        lsns.sort_unstable();
        assert_eq!(lsns, (0..(THREADS * PER_THREAD) as u64).collect::<Vec<_>>());

        // Each thread sees increasing LSNs in program order.
        for r in &results {
            assert!(r.windows(2).all(|w| w[0].0 < w[1].0));
        }

        // The record at file position `lsn` is the payload that got that LSN.
        let all = Wal::read_all(&path, MAX).unwrap();
        assert_eq!(all.len(), THREADS * PER_THREAD);
        for (lsn, p) in results.iter().flatten() {
            assert_eq!(&all[*lsn as usize], p);
        }
    });
}

/// xorshift64*: deterministic, no dependencies.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

#[test]
fn crash_simulation_any_file_prefix_recovers_a_record_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let reference = dir.path().join("reference");
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let payloads: Vec<Vec<u8>> = (0..60)
        .map(|_| {
            let n = rng.below(200) as usize;
            (0..n).map(|_| rng.next() as u8).collect()
        })
        .collect();
    write_log(&reference, &payloads);
    let ends = record_ends(&payloads);
    let bytes = fs::read(&reference).unwrap();

    let crashed = dir.path().join("crashed");
    for iteration in 0..200 {
        let k = rng.below(bytes.len() as u64 + 1) as usize;
        fs::write(&crashed, &bytes[..k]).unwrap();

        let (wal, rec) = Wal::open(&crashed, opts(SyncPolicy::Never)).unwrap();
        let expect = ends.iter().take_while(|&&e| e <= k as u64).count();
        assert_eq!(
            rec.records_recovered, expect as u64,
            "iter {iteration} k={k}"
        );
        let expect_len = if expect == 0 { 8 } else { ends[expect - 1] };
        assert_eq!(file_len(&crashed), expect_len, "iter {iteration} k={k}");
        if k >= 8 {
            assert_eq!(
                rec.bytes_truncated,
                k as u64 - expect_len,
                "iter {iteration}"
            );
        }
        assert_eq!(wal.append(b"tail").unwrap(), expect as u64);
        drop(wal);

        let all = Wal::read_all(&crashed, MAX).unwrap();
        assert_eq!(
            &all[..expect],
            &payloads[..expect],
            "iter {iteration} k={k}"
        );
        assert_eq!(all[expect], b"tail");
    }
}

#[test]
#[ignore = "throughput measurement; run with --release -- --ignored --nocapture"]
fn throughput_always_vs_every_100() {
    use std::time::Instant;
    let record = [0u8; 128];
    for (name, policy) in [
        ("Always", SyncPolicy::Always),
        ("EveryN(100)", SyncPolicy::EveryN(100)),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let (wal, _) = Wal::open(&dir.path().join("log"), opts(policy)).unwrap();
        let n = 2000;
        let start = Instant::now();
        for _ in 0..n {
            wal.append(&record).unwrap();
        }
        let secs = start.elapsed().as_secs_f64();
        println!(
            "{name}: {n} appends in {secs:.3} s = {:.0} appends/s, {} syncs",
            n as f64 / secs,
            wal.sync_count()
        );
    }
}
