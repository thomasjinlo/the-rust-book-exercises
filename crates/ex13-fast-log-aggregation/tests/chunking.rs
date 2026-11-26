mod common;

use common::{Rng, run, run_chunked};
use ex13_fast_log_aggregation::{Aggregator, baseline, datagen};

#[test]
fn generated_buffer_matches_baseline_for_every_chunk_size() {
    let data = datagen::generate(100_000, 42);
    let text = std::str::from_utf8(&data).expect("generator emits ASCII");
    let want = baseline::aggregate(text);

    assert_eq!(want.lines + want.malformed, 100_000);
    assert!(
        (1_000..=3_000).contains(&want.malformed),
        "about 2% malformed, got {}",
        want.malformed
    );
    assert!(want.slowest.is_some());

    for chunk in [1, 7, 4096, 65_536] {
        assert_eq!(run_chunked(&data, chunk), want, "chunk size {chunk}");
    }
    assert_eq!(run(&data), want, "whole buffer");
}

#[test]
fn random_chunk_boundaries_match_baseline() {
    let data = datagen::generate(5_000, 7);
    let want = baseline::aggregate(std::str::from_utf8(&data).unwrap());
    let mut rng = Rng(99);
    for _ in 0..20 {
        let mut agg = Aggregator::new();
        let mut rest = &data[..];
        while !rest.is_empty() {
            let n = (rng.below(300) as usize + 1).min(rest.len());
            agg.ingest(&rest[..n]);
            rest = &rest[n..];
        }
        assert_eq!(agg.finish(), want);
    }
}

#[test]
fn zero_length_chunks_are_ignored() {
    let data = datagen::generate(1_000, 3);
    let want = run(&data);
    let mut agg = Aggregator::new();
    agg.ingest(b"");
    for c in data.chunks(13) {
        agg.ingest(c);
        agg.ingest(b"");
    }
    assert_eq!(agg.finish(), want);
}

#[test]
fn mutated_input_matches_baseline() {
    // Replaces random bytes with characters that stress the parser: separators,
    // signs, digits, case changes and CR. Every case is checked against the
    // baseline and across chunkings.
    const NOISE: &[u8] = b" \n.-+0123456789ABGETxyz/\r:";
    let clean = datagen::generate(300, 1);
    let mut rng = Rng(2024);
    for case in 0..300 {
        let mut data = clean.clone();
        for _ in 0..(1 + rng.below(40)) {
            let i = rng.below(data.len() as u64) as usize;
            data[i] = NOISE[rng.below(NOISE.len() as u64) as usize];
        }
        if case % 3 == 0 {
            data.pop();
        }
        let want = baseline::aggregate(std::str::from_utf8(&data).unwrap());
        assert_eq!(run(&data), want, "case {case}, whole buffer");
        let chunk = 1 + rng.below(100) as usize;
        assert_eq!(
            run_chunked(&data, chunk),
            want,
            "case {case}, chunk {chunk}"
        );
    }
}
