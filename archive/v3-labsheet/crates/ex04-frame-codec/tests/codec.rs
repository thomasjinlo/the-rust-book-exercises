use ex04_frame_codec::*;

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
        encode(fr, &mut out);
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
    encode(&f(7, b"ab"), &mut out);
    assert_eq!(out, [0xEE, 0, 0, 0, 3, 7, b'a', b'b']);
}

#[test]
fn encode_empty_payload() {
    let mut out = Vec::new();
    encode(&f(9, b""), &mut out);
    assert_eq!(out, [0, 0, 0, 1, 9]);
}

#[test]
fn many_frames_in_one_chunk_come_out_in_order() {
    let frames = three();
    let mut d = Decoder::new(MAX);
    d.feed(&enc_all(&frames));
    assert_eq!(drain(&mut d), frames);
    assert_eq!(d.next_frame(), Ok(None));
}

#[test]
fn a_frame_without_payload_decodes_to_an_empty_payload() {
    let mut d = Decoder::new(MAX);
    d.feed(&[0, 0, 0, 1, 9]);
    assert_eq!(d.next_frame(), Ok(Some(f(9, b""))));
    assert_eq!(d.next_frame(), Ok(None));
}

#[test]
fn an_incomplete_frame_returns_none_until_the_rest_arrives() {
    let mut d = Decoder::new(MAX);
    d.feed(&[0, 0, 0]);
    assert_eq!(d.next_frame(), Ok(None), "length field incomplete");
    d.feed(&[2, 5]);
    assert_eq!(d.next_frame(), Ok(None), "length known, payload missing");
    assert_eq!(d.next_frame(), Ok(None), "asking again changes nothing");
    d.feed(&[6]);
    assert_eq!(d.next_frame(), Ok(Some(f(5, &[6]))));
    assert_eq!(d.next_frame(), Ok(None));
}

/// The worked example from the exercise text, one byte at a time.
#[test]
fn the_worked_example_decodes_one_byte_at_a_time() {
    let wire = [0, 0, 0, 3, 1, b'h', b'i', 0, 0, 0, 1, 2];
    let mut d = Decoder::new(MAX);
    for (i, b) in wire.iter().enumerate() {
        d.feed(&[*b]);
        let got = d.next_frame().unwrap();
        match i + 1 {
            7 => assert_eq!(got, Some(f(1, b"hi"))),
            12 => assert_eq!(got, Some(f(2, b""))),
            n => assert_eq!(got, None, "after byte {n}"),
        }
    }
}

#[test]
fn byte_at_a_time_decodes_every_frame() {
    let frames = three();
    let mut d = Decoder::new(MAX);
    let mut got = Vec::new();
    for b in enc_all(&frames) {
        d.feed(&[b]);
        got.extend(drain(&mut d));
    }
    assert_eq!(got, frames);
}

#[test]
fn every_split_point_gives_the_same_frames() {
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
fn a_zero_length_field_is_an_error() {
    let mut d = Decoder::new(MAX);
    d.feed(&[0, 0, 0, 0]);
    assert_eq!(d.next_frame(), Err(DecodeError::ZeroLength));
}

#[test]
fn an_oversize_length_is_reported_before_any_payload_arrives() {
    let mut d = Decoder::new(100);
    d.feed(&101u32.to_be_bytes());
    assert_eq!(
        d.next_frame(),
        Err(DecodeError::FrameTooLarge { len: 101, max: 100 })
    );
}

#[test]
fn a_length_equal_to_the_maximum_is_accepted() {
    let mut d = Decoder::new(4);
    d.feed(&[0, 0, 0, 4, 7, 1, 2, 3]);
    assert_eq!(d.next_frame(), Ok(Some(f(7, &[1, 2, 3]))));
}

#[test]
fn an_error_repeats_forever_and_later_frames_are_never_returned() {
    let mut d = Decoder::new(100);
    let mut wire = enc_all(&[f(1, b"a")]);
    wire.extend_from_slice(&[0, 0, 0, 0]);
    // A valid frame after the bad header must not be returned.
    encode(&f(2, b"b"), &mut wire);
    d.feed(&wire);
    assert_eq!(d.next_frame(), Ok(Some(f(1, b"a"))));
    assert_eq!(d.next_frame(), Err(DecodeError::ZeroLength));
    assert_eq!(d.next_frame(), Err(DecodeError::ZeroLength));
    d.feed(&enc_all(&[f(3, b"c")]));
    assert_eq!(d.next_frame(), Err(DecodeError::ZeroLength));
}

/// Small deterministic PRNG (xorshift64) so the test needs no crates.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u64) -> usize {
        (self.next() % n) as usize
    }
}

#[test]
fn random_chunking_gives_the_same_frames() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for _ in 0..200 {
        let frames: Vec<Frame> = (0..rng.below(20))
            .map(|_| {
                let payload: Vec<u8> = (0..rng.below(300)).map(|_| rng.next() as u8).collect();
                f(rng.next() as u8, &payload)
            })
            .collect();
        let wire = enc_all(&frames);
        let mut d = Decoder::new(1024);
        let mut got = Vec::new();
        let mut pos = 0;
        while pos < wire.len() {
            let n = (1 + rng.below(64)).min(wire.len() - pos);
            d.feed(&wire[pos..pos + n]);
            pos += n;
            got.extend(drain(&mut d));
        }
        assert_eq!(got, frames);
    }
}

#[test]
fn a_length_above_the_maximum_stays_an_error_after_more_bytes() {
    let mut d = Decoder::new(100);
    d.feed(&101u32.to_be_bytes());
    let err = Err(DecodeError::FrameTooLarge { len: 101, max: 100 });
    assert_eq!(d.next_frame(), err);
    d.feed(&[0u8; 500]);
    assert_eq!(d.next_frame(), err);
    assert_eq!(d.next_frame(), err);
}
