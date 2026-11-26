use ex03_frame_codec::*;
use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};

const MAX: u32 = 1 << 20;

fn f(kind: u8, payload: &[u8]) -> Frame {
    Frame {
        kind,
        payload: payload.to_vec(),
    }
}

fn enc_all(frames: &[Frame]) -> Vec<u8> {
    let mut out = Vec::new();
    for fr in frames {
        encode(fr, &mut out).unwrap();
    }
    out
}

/// Pulls frames until `Ok(None)`. Panics on error.
fn drain(d: &mut Decoder) -> Vec<Frame> {
    let mut v = Vec::new();
    while let Some(fr) = d.next_frame().unwrap() {
        v.push(fr);
    }
    v
}

fn three() -> Vec<Frame> {
    vec![f(1, b"hello"), f(2, b""), f(255, &[0, 1, 2, 3, 4, 5, 6, 7])]
}

#[test]
fn encode_exact_bytes() {
    let mut out = vec![0xEE];
    encode(&f(7, b"ab"), &mut out).unwrap();
    assert_eq!(out, [0xEE, 0, 0, 0, 3, 7, b'a', b'b']);
}

#[test]
fn round_trip() {
    let frames = three();
    let mut d = Decoder::new(MAX);
    d.feed(&enc_all(&frames));
    assert_eq!(drain(&mut d), frames);
    assert_eq!(d.buffered_len(), 0);
}

#[test]
fn split_at_every_byte_boundary() {
    let frames = three();
    let wire = enc_all(&frames);
    for cut in 0..=wire.len() {
        let mut d = Decoder::new(MAX);
        let mut got = Vec::new();
        d.feed(&wire[..cut]);
        got.extend(drain(&mut d));
        d.feed(&wire[cut..]);
        got.extend(drain(&mut d));
        assert_eq!(got, frames, "cut at {cut}");
    }
}

#[test]
fn byte_at_a_time() {
    let frames = three();
    let mut d = Decoder::new(MAX);
    let mut got = Vec::new();
    for b in enc_all(&frames) {
        d.feed(&[b]);
        got.extend(drain(&mut d));
    }
    assert_eq!(got, frames);
    assert_eq!(d.buffered_len(), 0);
}

#[test]
fn many_frames_in_one_feed() {
    let frames: Vec<Frame> = (0..1000u32)
        .map(|i| f((i % 256) as u8, &i.to_be_bytes()[..(i % 5) as usize]))
        .collect();
    let mut d = Decoder::new(MAX);
    d.feed(&enc_all(&frames));
    assert_eq!(drain(&mut d), frames);
}

#[test]
fn zero_length_is_an_error() {
    let mut d = Decoder::new(MAX);
    d.feed(&[0, 0, 0, 0]);
    assert_eq!(d.next_frame(), Err(DecodeError::ZeroLength));
}

#[test]
fn oversize_detected_after_four_bytes() {
    let mut d = Decoder::new(100);
    d.feed(&101u32.to_be_bytes());
    assert_eq!(
        d.next_frame(),
        Err(DecodeError::FrameTooLarge { len: 101, max: 100 })
    );
}

#[test]
fn partial_length_field_needs_more() {
    let mut d = Decoder::new(100);
    d.feed(&[0, 0, 0]);
    assert_eq!(d.next_frame(), Ok(None));
    assert_eq!(d.buffered_len(), 3);
}

#[test]
fn poisoning() {
    let mut d = Decoder::new(100);
    d.feed(&[0xFF, 0xFF, 0xFF, 0xFF]);
    let err = DecodeError::FrameTooLarge {
        len: u32::MAX,
        max: 100,
    };
    assert_eq!(d.next_frame(), Err(err.clone()));
    let held = d.buffered_len();
    // A valid frame after the error must not be decoded, and must not be stored.
    let mut ok = Vec::new();
    encode(&f(1, b"x"), &mut ok).unwrap();
    d.feed(&ok);
    assert_eq!(d.buffered_len(), held);
    assert_eq!(d.next_frame(), Err(err.clone()));
    assert_eq!(d.next_frame(), Err(err));
}

#[test]
fn poisoning_after_good_frames() {
    let mut d = Decoder::new(100);
    let mut wire = enc_all(&[f(1, b"a")]);
    wire.extend_from_slice(&[0, 0, 0, 0]);
    d.feed(&wire);
    assert_eq!(d.next_frame().unwrap().unwrap(), f(1, b"a"));
    assert_eq!(d.next_frame(), Err(DecodeError::ZeroLength));
    assert_eq!(d.next_frame(), Err(DecodeError::ZeroLength));
}

#[test]
fn empty_payload_frame() {
    let mut out = Vec::new();
    encode(&f(9, b""), &mut out).unwrap();
    assert_eq!(out, [0, 0, 0, 1, 9]);
    let mut d = Decoder::new(1);
    d.feed(&out);
    assert_eq!(d.next_frame(), Ok(Some(f(9, b""))));
    assert_eq!(d.next_frame(), Ok(None));
}

#[test]
fn max_size_boundary() {
    let max = 64u32;
    let at_max = f(3, &vec![0xAB; (max - 1) as usize]);
    let over = f(3, &vec![0xAB; max as usize]);

    let mut wire = Vec::new();
    encode_with_limit(&at_max, max, &mut wire).unwrap();
    let mut d = Decoder::new(max);
    d.feed(&wire);
    assert_eq!(d.next_frame(), Ok(Some(at_max)));

    let mut out = vec![1, 2, 3];
    assert_eq!(
        encode_with_limit(&over, max, &mut out),
        Err(EncodeError::PayloadTooLarge)
    );
    assert_eq!(out, [1, 2, 3], "out unchanged on error");
    // The same frame from the unlimited encoder is rejected by the decoder.
    let mut wire = Vec::new();
    encode(&over, &mut wire).unwrap();
    let mut d = Decoder::new(max);
    d.feed(&wire);
    assert_eq!(
        d.next_frame(),
        Err(DecodeError::FrameTooLarge { len: max + 1, max })
    );
}

#[test]
fn buffer_bound_with_large_frames() {
    let max = 1100u32;
    let frames: Vec<Frame> = (0..50u8)
        .map(|i| f(i, &vec![i; 990 + i as usize]))
        .collect();
    let wire = enc_all(&frames);
    let mut d = Decoder::new(max);
    let mut got = Vec::new();
    for c in wire.chunks(300) {
        d.feed(c);
        // Bound after feed: 4 + max + chunk length.
        assert!(d.buffered_len() <= 4 + max as usize + c.len());
        got.extend(drain(&mut d));
        // Bound after draining to Ok(None): less than 4 + max.
        assert!(d.buffered_len() < 4 + max as usize);
    }
    assert_eq!(got, frames);
}

#[test]
fn million_small_frames_do_not_grow_the_buffer() {
    let batch: Vec<Frame> = (0..1000u32).map(|i| f(i as u8, &i.to_be_bytes())).collect();
    let wire = enc_all(&batch); // 1000 frames of 9 bytes each

    let mut d = Decoder::new(MAX);
    let mut count = 0usize;
    for _ in 0..1000 {
        d.feed(&wire);
        while let Some(fr) = d.next_frame().unwrap() {
            assert_eq!(fr.payload.len(), 4);
            count += 1;
        }
        assert_eq!(d.buffered_len(), 0);
    }
    assert_eq!(count, 1_000_000);

    // Chunks that do not align with frames leave a partial frame behind.
    // After draining, fewer than 9 bytes (one frame) may remain.
    let mut d = Decoder::new(MAX);
    let big: Vec<u8> = wire
        .iter()
        .copied()
        .cycle()
        .take(wire.len() * 1000)
        .collect();
    let mut count = 0usize;
    let mut peak = 0usize;
    for c in big.chunks(4093) {
        d.feed(c);
        while d.next_frame().unwrap().is_some() {
            count += 1;
        }
        peak = peak.max(d.buffered_len());
    }
    assert_eq!(count, 1_000_000);
    assert!(peak < 9, "peak after draining was {peak}, expected < 9");
}

fn arb_frame() -> impl Strategy<Value = Frame> {
    (any::<u8>(), prop::collection::vec(any::<u8>(), 0..300))
        .prop_map(|(kind, payload)| Frame { kind, payload })
}

#[test]
fn proptest_random_chunking_round_trip() {
    // Fixed seed: the same cases run on every machine.
    let config = Config {
        cases: 256,
        failure_persistence: None,
        ..Config::default()
    };
    let rng = TestRng::from_seed(RngAlgorithm::ChaCha, &[7u8; 32]);
    let mut runner = TestRunner::new_with_rng(config, rng);
    let strat = (
        prop::collection::vec(arb_frame(), 0..20),
        prop::collection::vec(1usize..64, 1..50),
    );
    runner
        .run(&strat, |(frames, sizes)| {
            let wire = enc_all(&frames);
            let mut d = Decoder::new(1024);
            let mut got = Vec::new();
            let mut pos = 0;
            let mut i = 0;
            while pos < wire.len() {
                let n = sizes[i % sizes.len()].min(wire.len() - pos);
                i += 1;
                d.feed(&wire[pos..pos + n]);
                pos += n;
                while let Some(fr) = d.next_frame().unwrap() {
                    got.push(fr);
                }
            }
            prop_assert_eq!(got, frames);
            prop_assert_eq!(d.buffered_len(), 0);
            Ok(())
        })
        .unwrap();
}

#[test]
#[ignore = "throughput; run with: cargo test --release -- --ignored --nocapture"]
fn throughput_100_mib_of_1_kib_frames() {
    let one = f(1, &[0x5A; 1024]);
    let mut unit = Vec::new();
    encode(&one, &mut unit).unwrap();
    let frames_total = 100 * 1024 * 1024 / unit.len();
    let mut wire = Vec::with_capacity(unit.len() * 1000);
    for _ in 0..1000 {
        wire.extend_from_slice(&unit);
    }
    let mut d = Decoder::new(MAX);
    let mut count = 0usize;
    let start = std::time::Instant::now();
    let mut remaining = frames_total;
    while remaining > 0 {
        let n = remaining.min(1000);
        d.feed(&wire[..n * unit.len()]);
        while d.next_frame().unwrap().is_some() {
            count += 1;
        }
        remaining -= n;
    }
    let secs = start.elapsed().as_secs_f64();
    let mb = (count * unit.len()) as f64 / (1024.0 * 1024.0);
    println!(
        "decoded {count} frames, {mb:.1} MiB in {secs:.3} s = {:.0} MiB/s",
        mb / secs
    );
    assert_eq!(count, frames_total);
}
