use ex05_write_ahead_log::Wal;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT: AtomicU32 = AtomicU32::new(0);

/// A fresh path in the system temp dir, removed when dropped.
struct TempFile(PathBuf);

impl TempFile {
    fn new() -> Self {
        let n = NEXT.fetch_add(1, Ordering::SeqCst);
        TempFile(std::env::temp_dir().join(format!("wal-test-{}-{n}.log", std::process::id())))
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn write_log(path: &Path, records: &[&[u8]]) {
    let (mut wal, _) = Wal::open(path).unwrap();
    for r in records {
        wal.append(r).unwrap();
    }
}

fn reopen(path: &Path) -> Vec<Vec<u8>> {
    Wal::open(path).unwrap().1
}

fn as_slices(v: &[Vec<u8>]) -> Vec<&[u8]> {
    v.iter().map(|r| r.as_slice()).collect()
}

const THREE: [&[u8]; 3] = [b"alpha", b"", b"gamma"];

#[test]
fn a_missing_file_is_created_and_has_no_records() {
    let t = TempFile::new();
    let (_wal, records) = Wal::open(&t.0).unwrap();
    assert!(records.is_empty());
    assert!(t.0.exists());
}

#[test]
fn appended_records_come_back_in_order() {
    let t = TempFile::new();
    write_log(&t.0, &THREE);
    assert_eq!(as_slices(&reopen(&t.0)), THREE);
}

#[test]
fn an_appended_record_is_in_the_file_before_the_log_is_dropped() {
    let t = TempFile::new();
    let (mut wal, _) = Wal::open(&t.0).unwrap();
    wal.append(b"alpha").unwrap();
    assert!(fs::metadata(&t.0).unwrap().len() >= 5);
    assert_eq!(as_slices(&reopen(&t.0)), [b"alpha".as_slice()]);
    wal.append(b"beta").unwrap();
    assert_eq!(reopen(&t.0).len(), 2);
}

#[test]
fn records_can_hold_any_bytes() {
    let t = TempFile::new();
    let all: Vec<u8> = (0..=255).collect();
    write_log(&t.0, &[&all, &[0, 0, 0, 0], &[0xff; 3000]]);
    assert_eq!(
        as_slices(&reopen(&t.0)),
        [all.as_slice(), &[0, 0, 0, 0], &[0xff; 3000]]
    );
}

#[test]
fn reopening_and_appending_adds_to_the_end() {
    let t = TempFile::new();
    write_log(&t.0, &THREE);
    write_log(&t.0, &[b"delta"]);
    assert_eq!(
        as_slices(&reopen(&t.0)),
        [b"alpha".as_slice(), b"", b"gamma", b"delta"]
    );
}

#[test]
fn a_file_cut_inside_a_record_loses_that_record() {
    let t = TempFile::new();
    write_log(&t.0, &THREE);
    let full = fs::read(&t.0).unwrap();
    fs::write(&t.0, &full[..full.len() - 2]).unwrap();
    assert_eq!(
        as_slices(&reopen(&t.0)),
        [b"alpha".as_slice(), b""],
        "cut inside record 3"
    );
    fs::write(&t.0, []).unwrap();
    assert!(reopen(&t.0).is_empty(), "cut to 0 bytes");
}

#[test]
fn a_flipped_bit_drops_that_record_and_every_later_one() {
    let t = TempFile::new();
    write_log(&t.0, &THREE);
    let mut bytes = fs::read(&t.0).unwrap();
    // The last byte belongs to "gamma"; the records before it stay valid.
    *bytes.last_mut().unwrap() ^= 1;
    fs::write(&t.0, &bytes).unwrap();
    assert_eq!(as_slices(&reopen(&t.0)), [b"alpha".as_slice(), b""]);

    write_log(&t.0, &THREE);
    let mut bytes = fs::read(&t.0).unwrap();
    bytes[10] ^= 0x80;
    fs::write(&t.0, &bytes).unwrap();
    assert!(
        reopen(&t.0).is_empty(),
        "damage in record 1 drops records 2 and 3 too"
    );
}

/// Random records, cut or bit-flipped at a random place, then appended to.
#[test]
fn random_damage_then_appends_match_a_simple_model() {
    let mut rng = 0x2545_F491_4F6C_DD1Du64;
    let mut next = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    let t = TempFile::new();
    for _ in 0..100 {
        let _ = fs::remove_file(&t.0);
        let records: Vec<Vec<u8>> = (0..1 + next() % 12)
            .map(|_| (0..next() % 40).map(|_| next() as u8).collect())
            .collect();
        write_log(
            &t.0,
            &records.iter().map(|r| r.as_slice()).collect::<Vec<_>>(),
        );
        let mut bytes = fs::read(&t.0).unwrap();
        let at = (next() as usize) % bytes.len();
        if next() % 2 == 0 {
            bytes.truncate(at);
        } else {
            bytes[at] ^= 1 << (next() % 8);
        }
        fs::write(&t.0, &bytes).unwrap();

        let recovered = reopen(&t.0);
        assert!(recovered.len() <= records.len());
        assert_eq!(recovered, records[..recovered.len()]);

        write_log(&t.0, &[b"after"]);
        let mut expected = recovered;
        expected.push(b"after".to_vec());
        assert_eq!(reopen(&t.0), expected);
    }
}

/// Hard rule: cut the file at every length and flip every bit. Each time, `open` must return an
/// exact prefix of the appended records, and a later append must work.
/// Covers every cut length and every bit of the file (about 90 cuts and 700 flips). A correct
/// solution runs it in about 1.5 s in a debug build.
#[test]
fn any_cut_or_single_bit_flip_leaves_an_exact_prefix() {
    let records: [&[u8]; 5] = [b"alpha", b"", b"gamma", &[7; 20], b"epsilon"];
    let t = TempFile::new();
    write_log(&t.0, &records);
    let full = fs::read(&t.0).unwrap();

    let mut damaged: Vec<(String, Vec<u8>)> = Vec::new();
    for len in 0..=full.len() {
        damaged.push((format!("cut to {len}"), full[..len].to_vec()));
    }
    for byte in 0..full.len() {
        for bit in 0..8 {
            let mut f = full.clone();
            f[byte] ^= 1 << bit;
            damaged.push((format!("flip bit {bit} of byte {byte}"), f));
        }
    }

    for (what, bytes) in damaged {
        fs::write(&t.0, &bytes).unwrap();
        let recovered = reopen(&t.0);
        assert!(recovered.len() <= records.len(), "{what}: extra records");
        assert_eq!(
            as_slices(&recovered),
            records[..recovered.len()],
            "{what}: not a prefix"
        );

        write_log(&t.0, &[b"new"]);
        let again = reopen(&t.0);
        assert_eq!(again.len(), recovered.len() + 1, "{what}: after append");
        assert_eq!(again[..recovered.len()], recovered[..], "{what}");
        assert_eq!(again.last().unwrap(), b"new", "{what}");
    }
}
