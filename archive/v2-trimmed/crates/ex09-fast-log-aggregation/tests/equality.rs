mod common;

use common::Rng;
use ex09_fast_log_aggregation::{Aggregator, baseline, datagen};

#[test]
fn generated_buffer_matches_baseline() {
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
    assert_eq!(Aggregator::new().aggregate(&data), want);
}

#[test]
fn mutated_input_matches_baseline() {
    // Replaces random bytes with characters that stress the parser: separators,
    // signs, digits, case changes and CR. The seed is fixed.
    const NOISE: &[u8] = b" \n.-+0123456789ABGETxyz/\r:";
    let clean = datagen::generate(300, 1);
    let mut rng = Rng(2024);
    let mut agg = Aggregator::new();
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
        assert_eq!(agg.aggregate(&data), want, "case {case}");
    }
}
