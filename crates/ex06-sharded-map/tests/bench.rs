//! Throughput comparison. Run with:
//! `cargo test --release --test bench -- --ignored --nocapture`
use ex06_sharded_map::ShardedMap;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const KEYS: u64 = 10_000;
const RUN: Duration = Duration::from_millis(500);

trait Store: Sync {
    fn get(&self, k: u64) -> Option<u64>;
    fn put(&self, k: u64, v: u64);
}

impl Store for ShardedMap<u64, u64> {
    fn get(&self, k: u64) -> Option<u64> {
        ShardedMap::get(self, &k)
    }
    fn put(&self, k: u64, v: u64) {
        self.insert(k, v);
    }
}

impl Store for Mutex<HashMap<u64, u64>> {
    fn get(&self, k: u64) -> Option<u64> {
        self.lock().unwrap().get(&k).copied()
    }
    fn put(&self, k: u64, v: u64) {
        self.lock().unwrap().insert(k, v);
    }
}

/// 90% `get`, 10% `put`, uniform keys. Returns operations per second.
fn run(store: &dyn Store, threads: usize) -> f64 {
    let stop = AtomicBool::new(false);
    let total = AtomicU64::new(0);
    let start = Instant::now();
    thread::scope(|s| {
        for t in 0..threads {
            let (stop, total) = (&stop, &total);
            s.spawn(move || {
                let mut x = 0x9E37_79B9_7F4A_7C15u64 ^ (t as u64 + 1);
                let mut ops = 0u64;
                while !stop.load(Ordering::Relaxed) {
                    // Check the stop flag once per 1024 ops to keep it off the hot path.
                    for _ in 0..1024 {
                        x ^= x << 13;
                        x ^= x >> 7;
                        x ^= x << 17;
                        let key = (x >> 16) % KEYS;
                        if x.is_multiple_of(10) {
                            store.put(key, x);
                        } else {
                            std::hint::black_box(store.get(key));
                        }
                    }
                    ops += 1024;
                }
                total.fetch_add(ops, Ordering::Relaxed);
            });
        }
        thread::sleep(RUN);
        stop.store(true, Ordering::Relaxed);
    });
    total.load(Ordering::Relaxed) as f64 / start.elapsed().as_secs_f64()
}

#[test]
#[ignore = "benchmark; run with --release -- --ignored --nocapture"]
fn throughput_sharded_vs_single_mutex() {
    let sharded = ShardedMap::new(64).unwrap();
    let single = Mutex::new(HashMap::new());
    for k in 0..KEYS {
        sharded.insert(k, k);
        single.lock().unwrap().insert(k, k);
    }
    println!(
        "{:>8} {:>22} {:>22}",
        "threads", "sharded(64) ops/s", "Mutex<HashMap> ops/s"
    );
    for threads in [1, 2, 4, 8] {
        let a = run(&sharded, threads);
        let b = run(&single, threads);
        println!("{threads:>8} {a:>22.0} {b:>22.0}");
        assert!(a > 0.0 && b > 0.0);
    }
}
