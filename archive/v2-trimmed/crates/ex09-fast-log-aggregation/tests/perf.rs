//! Slow check. Run with: cargo test --release -- --ignored --nocapture

use std::time::Instant;

use ex09_fast_log_aggregation::{Aggregator, baseline, datagen};

/// Set below one third of the ratio observed on the verifying machine, so a
/// loaded host still passes.
const MIN_RATIO: f64 = 1.4;

fn best_of<F: FnMut()>(runs: usize, mut f: F) -> f64 {
    (0..runs)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed().as_secs_f64()
        })
        .fold(f64::INFINITY, f64::min)
}

#[test]
#[ignore = "slow: run with --release -- --ignored"]
fn aggregator_beats_baseline_throughput() {
    let data = datagen::generate(2_000_000, 2025);
    let text = std::str::from_utf8(&data).unwrap();
    let mb = data.len() as f64 / 1e6;

    let mut base_result = None;
    let base_s = best_of(3, || {
        base_result = Some(std::hint::black_box(baseline::aggregate(text)));
    });

    let mut agg = Aggregator::new();
    let mut fast_result = None;
    let fast_s = best_of(3, || {
        fast_result = Some(agg.aggregate(std::hint::black_box(&data)));
    });

    assert_eq!(fast_result, base_result, "results differ");
    let ratio = base_s / fast_s;
    println!("input: {mb:.1} MB, 2,000,000 lines");
    println!("baseline:   {:8.1} MB/s ({base_s:.3} s)", mb / base_s);
    println!("aggregator: {:8.1} MB/s ({fast_s:.3} s)", mb / fast_s);
    println!("ratio: {ratio:.2}x (threshold {MIN_RATIO}x)");
    assert!(ratio >= MIN_RATIO, "ratio {ratio:.2} < {MIN_RATIO}");
}
