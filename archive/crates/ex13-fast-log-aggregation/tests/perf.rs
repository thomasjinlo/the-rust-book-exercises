//! Slow check. Run with: cargo test --release -- --ignored --nocapture

use std::time::Instant;

use ex13_fast_log_aggregation::{Aggregator, baseline, datagen};

/// The reference solution measures about 3x this threshold on the
/// verifying machine, so this tolerates a heavily loaded host.
const MIN_RATIO: f64 = 1.5;

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
    let data = datagen::generate(5_000_000, 2025);
    let text = std::str::from_utf8(&data).unwrap();
    let mb = data.len() as f64 / 1e6;

    let mut base_result = None;
    let base_s = best_of(2, || {
        base_result = Some(std::hint::black_box(baseline::aggregate(text)));
    });

    let mut agg = Aggregator::new();
    let mut fast_result = None;
    let fast_s = best_of(3, || {
        for c in data.chunks(1 << 20) {
            agg.ingest(std::hint::black_box(c));
        }
        fast_result = Some(agg.finish());
    });

    assert_eq!(fast_result, base_result, "results differ");
    let ratio = base_s / fast_s;
    println!("input: {mb:.1} MB, 5,000,000 lines");
    println!("baseline:   {:8.1} MB/s ({base_s:.3} s)", mb / base_s);
    println!("aggregator: {:8.1} MB/s ({fast_s:.3} s)", mb / fast_s);
    println!("ratio: {ratio:.2}x (threshold {MIN_RATIO}x)");
    assert!(ratio >= MIN_RATIO, "ratio {ratio:.2} < {MIN_RATIO}");
}
