#![allow(dead_code)]

use ex13_fast_log_aggregation::{Aggregator, Summary};

/// Feeds `data` to a fresh aggregator in chunks of `chunk` bytes.
pub fn run_chunked(data: &[u8], chunk: usize) -> Summary {
    let mut agg = Aggregator::new();
    for c in data.chunks(chunk) {
        agg.ingest(c);
    }
    agg.finish()
}

pub fn run(data: &[u8]) -> Summary {
    run_chunked(data, data.len().max(1))
}

/// Small deterministic PRNG for randomized tests.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}
