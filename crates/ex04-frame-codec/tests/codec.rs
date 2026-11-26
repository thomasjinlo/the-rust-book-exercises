use ex04_frame_codec::{Decoder, TooLarge, encode};

fn stream(payloads: &[&[u8]]) -> Vec<u8> {
    let mut out = Vec::new();
    for p in payloads {
        encode(p, &mut out);
    }
    out
}

fn drain(d: &mut Decoder) -> Vec<Vec<u8>> {
    let mut frames = Vec::new();
    while let Some(f) = d.next_frame().unwrap() {
        frames.push(f);
    }
    frames
}

#[test]
fn encode_writes_a_big_endian_length_then_the_payload() {
    let mut out = Vec::new();
    encode(b"hi", &mut out);
    assert_eq!(out, [0, 0, 0, 2, 0x68, 0x69]);
    let mut long = Vec::new();
    encode(&[7; 258], &mut long);
    assert_eq!(long[..4], [0, 0, 1, 2]);
    assert_eq!(long.len(), 262);
}

#[test]
fn encode_appends_and_keeps_existing_bytes() {
    let mut out = vec![0xEE];
    encode(b"a", &mut out);
    assert_eq!(out, [0xEE, 0, 0, 0, 1, 0x61]);
}

#[test]
fn a_whole_frame_decodes_to_its_payload() {
    let mut d = Decoder::new(1024);
    d.feed(&stream(&[b"hello"]));
    assert_eq!(d.next_frame(), Ok(Some(b"hello".to_vec())));
    assert_eq!(d.next_frame(), Ok(None));
}

#[test]
fn bytes_fed_one_at_a_time_give_a_frame_only_when_it_is_complete() {
    // Two frames, "hi" and empty: 00 00 00 02 68 69 | 00 00 00 00.
    let bytes = stream(&[b"hi", b""]);
    assert_eq!(bytes.len(), 10);
    let mut d = Decoder::new(1024);
    for (i, b) in bytes.iter().enumerate() {
        d.feed(&[*b]);
        let got = d.next_frame().unwrap();
        match i + 1 {
            6 => assert_eq!(got, Some(b"hi".to_vec())),
            10 => assert_eq!(got, Some(vec![])),
            n => assert_eq!(got, None, "after byte {n}"),
        }
    }
    assert_eq!(d.next_frame(), Ok(None));
}

#[test]
fn several_frames_in_one_chunk_come_out_in_order() {
    let mut d = Decoder::new(1024);
    d.feed(&stream(&[b"a", b"bb", b"ccc"]));
    assert_eq!(
        drain(&mut d),
        [b"a".to_vec(), b"bb".to_vec(), b"ccc".to_vec()]
    );
}

#[test]
fn an_empty_frame_is_valid() {
    let mut d = Decoder::new(0);
    d.feed(&[0, 0, 0, 0]);
    assert_eq!(d.next_frame(), Ok(Some(vec![])));
    assert_eq!(d.next_frame(), Ok(None));
}

#[test]
fn a_frame_of_exactly_max_len_is_accepted() {
    let mut d = Decoder::new(3);
    d.feed(&stream(&[b"abc"]));
    assert_eq!(d.next_frame(), Ok(Some(b"abc".to_vec())));
}

#[test]
fn a_too_large_length_is_an_error_as_soon_as_its_four_bytes_arrive_and_stays_one() {
    let mut d = Decoder::new(100);
    d.feed(&[0, 0]);
    assert_eq!(d.next_frame(), Ok(None));
    d.feed(&[0, 101]);
    assert_eq!(d.next_frame(), Err(TooLarge));
    d.feed(&[0; 500]);
    assert_eq!(d.next_frame(), Err(TooLarge));
    assert_eq!(d.next_frame(), Err(TooLarge));
}

#[test]
fn any_chunking_of_a_stream_gives_the_same_frames() {
    let payloads: [&[u8]; 4] = [b"hi", b"", b"a longer payload", b"z"];
    let bytes = stream(&payloads);
    let expected: Vec<Vec<u8>> = payloads.iter().map(|p| p.to_vec()).collect();

    // Every single split point.
    for cut in 0..=bytes.len() {
        let mut d = Decoder::new(1024);
        let mut got = Vec::new();
        for part in [&bytes[..cut], &bytes[cut..]] {
            d.feed(part);
            got.extend(drain(&mut d));
        }
        assert_eq!(got, expected, "split at {cut}");
    }

    // One byte at a time.
    let mut d = Decoder::new(1024);
    let mut got = Vec::new();
    for b in &bytes {
        d.feed(&[*b]);
        got.extend(drain(&mut d));
    }
    assert_eq!(got, expected, "byte at a time");

    // Seeded random chunk sizes (xorshift64), including empty chunks.
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..200 {
        let mut d = Decoder::new(1024);
        let mut got = Vec::new();
        let mut pos = 0;
        while pos < bytes.len() {
            let n = (next() % 9) as usize;
            let end = (pos + n).min(bytes.len());
            d.feed(&bytes[pos..end]);
            got.extend(drain(&mut d));
            pos = end;
        }
        assert_eq!(got, expected, "random chunks");
    }
}
