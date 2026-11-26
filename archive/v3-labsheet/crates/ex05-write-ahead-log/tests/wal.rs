//! Black-box tests: they use `Wal::open`, `Wal::append`, `Wal::read_all` and
//! look at the log file only as bytes (its length and its content).

use ex05_write_ahead_log::Wal;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const MAX: u32 = 1 << 20;

/// Runs the test body on its own thread so a looping implementation fails
/// the test instead of hanging `cargo test`. The limit is far above the
/// few seconds the reference solution needs.
fn guarded(body: impl FnOnce() + Send + 'static) {
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        body();
        let _ = tx.send(());
    });
    match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(()) => handle.join().unwrap(),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            std::panic::resume_unwind(handle.join().unwrap_err())
        }
        Err(mpsc::RecvTimeoutError::Timeout) => panic!("test did not finish within 120 s"),
    }
}

/// Small deterministic generator (xorshift64*), so every run sees the same data.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn bytes(&mut self, len: usize) -> Vec<u8> {
        (0..len).map(|_| self.next() as u8).collect()
    }
}

fn open(path: &Path) -> (Wal, u64) {
    let (wal, recovery) = Wal::open(path, MAX).unwrap();
    (wal, recovery.records_recovered)
}

/// A log with six records of different sizes (0 to 40 bytes).
struct Sample {
    records: Vec<Vec<u8>>,
    /// `ends[0]` is the file length right after the first `open`. `ends[i]`
    /// is the file length right after record `i - 1` was appended.
    ends: Vec<u64>,
    bytes: Vec<u8>,
}

fn sample(path: &Path) -> Sample {
    let mut rng = Rng(7);
    let records: Vec<Vec<u8>> = [0, 1, 5, 40, 3, 17].iter().map(|&n| rng.bytes(n)).collect();
    let (mut wal, recovered) = open(path);
    assert_eq!(recovered, 0);
    let mut ends = vec![fs::metadata(path).unwrap().len()];
    for (i, record) in records.iter().enumerate() {
        assert_eq!(wal.append(record).unwrap(), i as u64);
        ends.push(fs::metadata(path).unwrap().len());
    }
    drop(wal);
    let bytes = fs::read(path).unwrap();
    assert_eq!(bytes.len() as u64, *ends.last().unwrap());
    Sample {
        records,
        ends,
        bytes,
    }
}

fn assert_is_prefix(got: &[Vec<u8>], original: &[Vec<u8>], context: &str) {
    assert!(
        got.len() <= original.len() && got == &original[..got.len()],
        "{context}: recovered {} records that are not a prefix of the {} written",
        got.len(),
        original.len()
    );
}

// ---- Milestone 1: append, reopen, read ----

#[test]
fn records_survive_reopen_and_sequence_numbers_continue() {
    guarded(|| {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log");
        let mut rng = Rng(1);
        let mut records: Vec<Vec<u8>> = (0..19).map(|i| rng.bytes(i * 7)).collect();
        records.push(rng.bytes(100_000));

        let (mut wal, recovered) = open(&path);
        assert_eq!(recovered, 0);
        for (i, r) in records.iter().enumerate() {
            assert_eq!(wal.append(r).unwrap(), i as u64);
        }
        drop(wal);

        let (mut wal, recovered) = open(&path);
        assert_eq!(recovered, 20);
        assert_eq!(wal.append(b"after reopen").unwrap(), 20);
        drop(wal);

        records.push(b"after reopen".to_vec());
        assert_eq!(Wal::read_all(&path, MAX).unwrap(), records);
        let (_wal, recovered) = open(&path);
        assert_eq!(recovered, 21);
    });
}

#[test]
fn an_appended_record_is_in_the_file_when_append_returns() {
    guarded(|| {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log");
        let (mut wal, _) = open(&path);
        let mut before = fs::metadata(&path).unwrap().len();
        let mut expected: Vec<Vec<u8>> = Vec::new();
        for i in 0..10u8 {
            let record = vec![i; usize::from(i) * 3];
            wal.append(&record).unwrap();
            expected.push(record);
            // The handle is still open: another reader must already see it.
            assert_eq!(Wal::read_all(&path, MAX).unwrap(), expected);
            let now = fs::metadata(&path).unwrap().len();
            assert!(now > before, "file did not grow after append {i}");
            before = now;
        }
    });
}

#[test]
fn a_missing_or_empty_file_opens_as_an_empty_log() {
    guarded(|| {
        let dir = tempfile::tempdir().unwrap();

        let missing = dir.path().join("missing");
        let err = Wal::read_all(&missing, MAX).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert!(!missing.exists(), "read_all must not create the file");
        let (mut wal, recovered) = open(&missing);
        assert_eq!(recovered, 0);
        assert_eq!(wal.append(b"first").unwrap(), 0);
        drop(wal);
        assert_eq!(
            Wal::read_all(&missing, MAX).unwrap(),
            vec![b"first".to_vec()]
        );

        let empty = dir.path().join("empty");
        fs::write(&empty, b"").unwrap();
        assert_eq!(Wal::read_all(&empty, MAX).unwrap(), Vec::<Vec<u8>>::new());
        let (mut wal, recovered) = open(&empty);
        assert_eq!(recovered, 0);
        assert_eq!(wal.append(b"first").unwrap(), 0);
        drop(wal);
        assert_eq!(Wal::read_all(&empty, MAX).unwrap(), vec![b"first".to_vec()]);
    });
}

#[test]
fn opening_a_clean_log_leaves_the_file_unchanged() {
    guarded(|| {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log");
        let s = sample(&path);
        for _ in 0..5 {
            let (wal, recovered) = open(&path);
            assert_eq!(recovered, 6);
            drop(wal);
            assert_eq!(fs::read(&path).unwrap(), s.bytes);
        }
    });
}

#[test]
fn many_open_append_close_cycles_keep_every_record() {
    guarded(|| {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log");
        let mut rng = Rng(99);
        let mut model: Vec<Vec<u8>> = Vec::new();
        let mut last_len = 0;
        for cycle in 0..60usize {
            let (mut wal, recovered) = open(&path);
            assert_eq!(recovered, model.len() as u64, "cycle {cycle}");
            for _ in 0..1 + cycle % 4 {
                let len = (rng.next() % 200) as usize;
                let record = rng.bytes(len);
                assert_eq!(wal.append(&record).unwrap(), model.len() as u64);
                model.push(record);
            }
            drop(wal);
            let len = fs::metadata(&path).unwrap().len();
            assert!(len > last_len, "file did not grow in cycle {cycle}");
            last_len = len;
            assert_eq!(Wal::read_all(&path, MAX).unwrap(), model, "cycle {cycle}");
        }
    });
}

#[test]
fn read_all_leaves_a_damaged_file_untouched() {
    guarded(|| {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log");
        let s = sample(&path);
        // Cut the last record in half and put garbage after the cut.
        let cut = (s.ends[5] + s.ends[6]) as usize / 2;
        let mut damaged = s.bytes[..cut].to_vec();
        damaged.extend_from_slice(b"garbage after the cut");
        fs::write(&path, &damaged).unwrap();

        assert_eq!(Wal::read_all(&path, MAX).unwrap(), s.records[..5].to_vec());
        assert_eq!(fs::read(&path).unwrap(), damaged);
    });
}

// ---- Milestone 2: torn tail ----

#[test]
fn a_log_cut_at_any_length_recovers_exactly_the_records_that_were_complete() {
    guarded(|| {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("original");
        let s = sample(&original);
        let path = dir.path().join("cut");

        for cut in 0..=s.bytes.len() {
            // A record is complete when the cut keeps all of its bytes.
            let complete = s.ends[1..].iter().filter(|&&e| e <= cut as u64).count();
            let expected = &s.records[..complete];
            let context = format!("cut at {cut} of {}", s.bytes.len());

            fs::write(&path, &s.bytes[..cut]).unwrap();
            assert_eq!(Wal::read_all(&path, MAX).unwrap(), expected, "{context}");
            assert_eq!(fs::read(&path).unwrap(), &s.bytes[..cut], "{context}");

            let (mut wal, recovered) = open(&path);
            assert_eq!(recovered, complete as u64, "{context}");
            assert_eq!(wal.append(b"new").unwrap(), complete as u64, "{context}");
            drop(wal);

            let mut with_new = expected.to_vec();
            with_new.push(b"new".to_vec());
            assert_eq!(Wal::read_all(&path, MAX).unwrap(), with_new, "{context}");
            let (_wal, recovered) = open(&path);
            assert_eq!(recovered, complete as u64 + 1, "{context}");
        }
    });
}

// ---- Milestone 3: corruption ----

#[test]
fn a_log_with_any_single_flipped_bit_recovers_a_prefix_of_the_records() {
    guarded(|| {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("original");
        let s = sample(&original);
        let path = dir.path().join("flipped");

        // Every bit of every byte of the file, one flip at a time.
        for pos in 0..s.bytes.len() {
            for bit in 0..8 {
                let mut damaged = s.bytes.clone();
                damaged[pos] ^= 1 << bit;
                fs::write(&path, &damaged).unwrap();
                let context = format!("bit {bit} of byte {pos}");
                // Damage to the first 16 bytes may make open fail. Damage
                // anywhere later must not.
                let may_fail = pos < 16;

                let read = Wal::read_all(&path, MAX);
                if let Err(e) = &read {
                    assert!(may_fail, "{context}: read_all failed: {e}");
                }
                assert_eq!(fs::read(&path).unwrap(), damaged, "{context}");
                let got = read.unwrap_or_default();
                assert_is_prefix(&got, &s.records, &context);
                // Records that end before the damaged byte are intact. The
                // damaged byte lies in record `k`, so records 0..k must stay.
                // This applies when read_all succeeded, or the byte is past
                // the file's first 16 bytes.
                if (!may_fail || !got.is_empty())
                    && let Some(k) = s.ends[..6].iter().rposition(|&e| e <= pos as u64)
                {
                    assert!(got.len() >= k, "{context}: lost earlier records");
                }

                let (mut wal, recovered) = match Wal::open(&path, MAX) {
                    Ok(opened) => (opened.0, opened.1.records_recovered),
                    Err(e) => {
                        assert!(may_fail, "{context}: open failed: {e}");
                        continue;
                    }
                };
                assert_eq!(recovered as usize, got.len(), "{context}");
                assert_eq!(wal.append(b"tail").unwrap(), recovered, "{context}");
                drop(wal);
                let mut expected = got.clone();
                expected.push(b"tail".to_vec());
                assert_eq!(Wal::read_all(&path, MAX).unwrap(), expected, "{context}");
                let (_wal, again) = open(&path);
                assert_eq!(again, recovered + 1, "{context}");
            }
        }
    });
}

// ---- Milestone 4: foreign files ----

#[test]
fn a_file_of_unrelated_content_is_rejected_and_left_unchanged() {
    guarded(|| {
        let dir = tempfile::tempdir().unwrap();
        let text = "The quick brown fox jumps over the lazy dog. \
                    This file is a plain text note and has nothing to do with any log. \
                    It is long enough to be more than a damaged header."
            .as_bytes()
            .to_vec();
        let noise = Rng(5).bytes(300);
        for (name, content) in [("text", text), ("noise", noise)] {
            let path = dir.path().join(name);
            fs::write(&path, &content).unwrap();
            let err = Wal::open(&path, MAX).err().expect("open must fail");
            assert_eq!(err.kind(), ErrorKind::InvalidData, "{name}");
            let err = Wal::read_all(&path, MAX).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::InvalidData, "{name}");
            assert_eq!(fs::read(&path).unwrap(), content, "{name}");
        }
    });
}

// ---- Milestone 5: record size limit ----

#[test]
fn append_rejects_payloads_over_the_limit_without_using_a_sequence_number() {
    guarded(|| {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log");
        let (mut wal, _) = Wal::open(&path, 10).unwrap();
        assert_eq!(wal.append(&[1; 10]).unwrap(), 0);
        let len = fs::metadata(&path).unwrap().len();

        let err = wal.append(&[2; 11]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert_eq!(fs::metadata(&path).unwrap().len(), len);

        assert_eq!(wal.append(b"").unwrap(), 1);
        assert_eq!(wal.append(&[3; 10]).unwrap(), 2);
        drop(wal);
        assert_eq!(
            Wal::read_all(&path, 10).unwrap(),
            vec![vec![1; 10], vec![], vec![3; 10]]
        );
        let (_wal, recovery) = Wal::open(&path, 10).unwrap();
        assert_eq!(recovery.records_recovered, 3);
    });
}

#[test]
fn reopening_with_a_smaller_limit_drops_longer_records_and_everything_after() {
    guarded(|| {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log");
        let records: Vec<Vec<u8>> = vec![vec![1; 3], vec![2; 10], vec![3; 4], vec![4; 3]];
        let (mut wal, _) = open(&path);
        for r in &records {
            wal.append(r).unwrap();
        }
        drop(wal);
        let bytes = fs::read(&path).unwrap();

        // The second record is over the new limit of 5, so the third and
        // fourth go with it even though they are small enough.
        assert_eq!(Wal::read_all(&path, 5).unwrap(), records[..1].to_vec());
        assert_eq!(fs::read(&path).unwrap(), bytes);

        let (mut wal, recovery) = Wal::open(&path, 5).unwrap();
        assert_eq!(recovery.records_recovered, 1);
        assert_eq!(wal.append(b"new").unwrap(), 1);
        drop(wal);

        let expected = vec![records[0].clone(), b"new".to_vec()];
        assert_eq!(Wal::read_all(&path, MAX).unwrap(), expected);
    });
}
