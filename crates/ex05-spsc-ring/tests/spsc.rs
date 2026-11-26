use ex05_spsc_ring::{Consumer, Producer, channel};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Runs `f` on a thread and fails the test if it panics or exceeds `secs`.
/// Limits are at least 10x the expected debug-build run time.
fn watchdog<F: FnOnce() + Send + 'static>(secs: u64, f: F) {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        f();
        let _ = tx.send(());
    });
    match rx.recv_timeout(Duration::from_secs(secs)) {
        Ok(()) => {}
        Err(mpsc::RecvTimeoutError::Disconnected) => panic!("test body panicked"),
        Err(mpsc::RecvTimeoutError::Timeout) => panic!("test body exceeded {secs}s"),
    }
}

/// Increments a shared counter on drop and carries an id for ordering checks.
struct Counted {
    id: u64,
    drops: Arc<AtomicUsize>,
}

impl Counted {
    fn new(id: u64, drops: &Arc<AtomicUsize>) -> Self {
        Counted {
            id,
            drops: Arc::clone(drops),
        }
    }
}

impl Drop for Counted {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn capacity_rounding() {
    watchdog(30, || {
        let table = [
            (0, 1),
            (1, 1),
            (2, 2),
            (3, 4),
            (4, 4),
            (5, 8),
            (63, 64),
            (64, 64),
            (65, 128),
            (1000, 1024),
        ];
        for (min, want) in table {
            let (p, c) = channel::<u8>(min);
            assert_eq!(p.capacity(), want, "min_capacity {min}");
            assert_eq!(c.capacity(), want, "min_capacity {min}");
        }
    });
}

#[test]
fn fifo_single_thread() {
    watchdog(30, || {
        let (mut p, mut c) = channel::<u32>(8);
        assert!(c.is_empty());
        assert_eq!(c.pop(), None);
        for i in 0..5 {
            p.push(i).unwrap();
        }
        assert!(!c.is_empty());
        for i in 0..5 {
            assert_eq!(c.pop(), Some(i));
        }
        assert_eq!(c.pop(), None);
        assert!(c.is_empty());
    });
}

#[test]
fn full_returns_value() {
    watchdog(30, || {
        let (mut p, mut c) = channel::<String>(4);
        assert!(!p.is_full());
        for i in 0..4 {
            p.push(format!("v{i}")).unwrap();
        }
        assert!(p.is_full());
        assert_eq!(p.push("rejected".to_string()), Err("rejected".to_string()));
        assert_eq!(c.pop().as_deref(), Some("v0"));
        assert!(!p.is_full());
        p.push("v4".to_string()).unwrap();
        let rest: Vec<String> = std::iter::from_fn(|| c.pop()).collect();
        assert_eq!(rest, ["v1", "v2", "v3", "v4"]);
    });
}

#[test]
fn wraparound_ten_times_capacity() {
    watchdog(30, || {
        let (mut p, mut c) = channel::<usize>(8);
        // Bursts of varying length keep head and tail at different offsets.
        let mut next_push = 0usize;
        let mut next_pop = 0usize;
        while next_pop < 80 {
            let burst = 1 + next_pop % 8;
            for _ in 0..burst {
                if p.push(next_push).is_ok() {
                    next_push += 1;
                }
            }
            for _ in 0..burst {
                if let Some(v) = c.pop() {
                    assert_eq!(v, next_pop);
                    next_pop += 1;
                }
            }
        }
    });
}

#[test]
fn fill_drain_cycles_after_wrap() {
    watchdog(30, || {
        let (mut p, mut c) = channel::<u64>(4);
        for round in 0..25u64 {
            for i in 0..4 {
                p.push(round * 4 + i).unwrap();
            }
            assert!(p.push(u64::MAX).is_err());
            for i in 0..4 {
                assert_eq!(c.pop(), Some(round * 4 + i));
            }
            assert_eq!(c.pop(), None);
        }
    });
}

#[test]
fn drop_counts_popped_items() {
    watchdog(30, || {
        let drops = Arc::new(AtomicUsize::new(0));
        let (mut p, mut c) = channel::<Counted>(8);
        for i in 0..6 {
            p.push(Counted::new(i, &drops)).ok().unwrap();
        }
        for i in 0..4 {
            let item = c.pop().unwrap();
            assert_eq!(item.id, i);
        }
        assert_eq!(
            drops.load(Ordering::SeqCst),
            4,
            "popped items drop once, at the caller"
        );
        drop(p);
        drop(c);
        assert_eq!(
            drops.load(Ordering::SeqCst),
            6,
            "2 remaining items drop with the queue"
        );
    });
}

#[test]
fn rejected_push_is_not_dropped_by_queue() {
    watchdog(30, || {
        let drops = Arc::new(AtomicUsize::new(0));
        let (mut p, c) = channel::<Counted>(2);
        p.push(Counted::new(0, &drops)).ok().unwrap();
        p.push(Counted::new(1, &drops)).ok().unwrap();
        let back = p.push(Counted::new(2, &drops)).err().unwrap();
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert_eq!(back.id, 2);
        drop(back);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        drop(p);
        drop(c);
        assert_eq!(drops.load(Ordering::SeqCst), 3);
    });
}

fn leftover_after_drop(producer_first: bool) {
    let drops = Arc::new(AtomicUsize::new(0));
    // Wrap the indices first so the remaining items straddle the buffer end.
    let (mut p, mut c) = channel::<Counted>(4);
    for i in 0..3 {
        p.push(Counted::new(i, &drops)).ok().unwrap();
    }
    for _ in 0..3 {
        drop(c.pop().unwrap());
    }
    assert_eq!(drops.load(Ordering::SeqCst), 3);
    for i in 3..7 {
        p.push(Counted::new(i, &drops)).ok().unwrap();
    }
    if producer_first {
        drop(p);
        assert_eq!(
            drops.load(Ordering::SeqCst),
            3,
            "queue lives while the consumer lives"
        );
        assert_eq!(c.pop().map(|x| x.id), Some(3));
        assert_eq!(drops.load(Ordering::SeqCst), 4);
        drop(c);
    } else {
        drop(c);
        assert_eq!(
            drops.load(Ordering::SeqCst),
            3,
            "queue lives while the producer lives"
        );
        drop(p);
    }
    assert_eq!(
        drops.load(Ordering::SeqCst),
        7,
        "every item dropped exactly once"
    );
}

#[test]
fn drop_order_producer_first() {
    watchdog(30, || leftover_after_drop(true));
}

#[test]
fn drop_order_consumer_first() {
    watchdog(30, || leftover_after_drop(false));
}

#[test]
fn drop_empty_and_full_queues() {
    watchdog(30, || {
        let drops = Arc::new(AtomicUsize::new(0));
        let (p, c) = channel::<Counted>(4);
        drop(p);
        drop(c);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        let (mut p, c) = channel::<Counted>(4);
        for i in 0..4 {
            p.push(Counted::new(i, &drops)).ok().unwrap();
        }
        drop(c);
        drop(p);
        assert_eq!(drops.load(Ordering::SeqCst), 4);
    });
}

fn spsc_sequence(total: u64, cap: usize) {
    let (mut p, mut c) = channel::<u64>(cap);
    let producer = thread::spawn(move || {
        for i in 0..total {
            let mut v = i;
            while let Err(back) = p.push(v) {
                v = back;
                std::hint::spin_loop();
            }
        }
    });
    let consumer = thread::spawn(move || {
        let mut expect = 0u64;
        while expect < total {
            match c.pop() {
                Some(v) => {
                    assert_eq!(v, expect, "out of order or corrupted");
                    expect += 1;
                }
                None => std::hint::spin_loop(),
            }
        }
        assert_eq!(c.pop(), None);
    });
    producer.join().unwrap();
    consumer.join().unwrap();
}

#[test]
fn two_thread_stress_500k() {
    watchdog(120, || spsc_sequence(500_000, 1024));
}

#[test]
fn two_thread_stress_capacity_2() {
    // A tiny buffer stays near full or empty, which maximizes index contention.
    watchdog(120, || spsc_sequence(200_000, 2));
}

#[test]
#[ignore = "release benchmark: cargo test --release -- --ignored --nocapture"]
fn two_thread_stress_20m_release() {
    let start = Instant::now();
    spsc_sequence(20_000_000, 1024);
    let secs = start.elapsed().as_secs_f64();
    println!("spsc: {:.1} M ops/sec", 20.0 / secs);
}

#[test]
fn two_thread_heap_payloads_counted() {
    watchdog(120, || {
        let drops = Arc::new(AtomicUsize::new(0));
        let total = 100_000u64;
        let (mut p, mut c) = channel::<(Counted, Box<u64>, String)>(64);
        let d = Arc::clone(&drops);
        let producer = thread::spawn(move || {
            for i in 0..total {
                let mut v = (Counted::new(i, &d), Box::new(i * 3), format!("msg-{i}"));
                while let Err(back) = p.push(v) {
                    v = back;
                    std::hint::spin_loop();
                }
            }
        });
        let consumer = thread::spawn(move || {
            let mut expect = 0u64;
            while expect < total {
                if let Some((n, b, s)) = c.pop() {
                    assert_eq!(n.id, expect);
                    assert_eq!(*b, expect * 3);
                    assert_eq!(s, format!("msg-{expect}"));
                    expect += 1;
                }
            }
        });
        producer.join().unwrap();
        consumer.join().unwrap();
        assert_eq!(drops.load(Ordering::SeqCst), total as usize);
    });
}

#[test]
fn leftovers_dropped_once_across_threads() {
    watchdog(60, || {
        let drops = Arc::new(AtomicUsize::new(0));
        let (mut p, mut c) = channel::<Counted>(16);
        let d = Arc::clone(&drops);
        let p = thread::spawn(move || {
            for i in 0..16 {
                p.push(Counted::new(i, &d)).ok().unwrap();
            }
            p
        })
        .join()
        .unwrap();
        for _ in 0..5 {
            drop(c.pop().unwrap());
        }
        thread::spawn(move || drop(c)).join().unwrap();
        assert_eq!(drops.load(Ordering::SeqCst), 5);
        drop(p);
        assert_eq!(drops.load(Ordering::SeqCst), 16);
    });
}

#[test]
fn zero_sized_type() {
    watchdog(30, || {
        let (mut p, mut c) = channel::<()>(4);
        for _ in 0..4 {
            p.push(()).unwrap();
        }
        assert_eq!(p.push(()), Err(()));
        for _ in 0..4 {
            assert_eq!(c.pop(), Some(()));
        }
        assert_eq!(c.pop(), None);
    });
}

#[test]
fn capacity_one_ping_pong_1m() {
    watchdog(120, || {
        let (mut req_tx, mut req_rx) = channel::<u64>(0);
        let (mut resp_tx, mut resp_rx) = channel::<u64>(1);
        assert_eq!(req_tx.capacity(), 1);
        assert_eq!(resp_tx.capacity(), 1);
        let n = 1_000_000u64;
        let echo = thread::spawn(move || {
            for _ in 0..n {
                let v = loop {
                    if let Some(v) = req_rx.pop() {
                        break v;
                    }
                    std::hint::spin_loop();
                };
                while resp_tx.push(v + 1).is_err() {
                    std::hint::spin_loop();
                }
            }
        });
        for i in 0..n {
            while req_tx.push(i).is_err() {
                std::hint::spin_loop();
            }
            let back = loop {
                if let Some(v) = resp_rx.pop() {
                    break v;
                }
                std::hint::spin_loop();
            };
            assert_eq!(back, i + 1);
        }
        echo.join().unwrap();
    });
}

/// xorshift64: the same schedule on every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

#[test]
fn randomized_model_check_single_thread() {
    watchdog(60, || {
        for seed in 1..=20u64 {
            let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
            let min = (rng.next() % 40) as usize;
            let (mut p, mut c) = channel::<u64>(min);
            let cap = p.capacity();
            let mut model = std::collections::VecDeque::new();
            for step in 0..5_000u64 {
                if rng.next().is_multiple_of(2) {
                    let r = p.push(step);
                    if model.len() == cap {
                        assert_eq!(r, Err(step));
                    } else {
                        assert_eq!(r, Ok(()));
                        model.push_back(step);
                    }
                    assert_eq!(p.is_full(), model.len() == cap);
                } else {
                    assert_eq!(c.pop(), model.pop_front());
                    assert_eq!(c.is_empty(), model.is_empty());
                }
            }
        }
    });
}

#[test]
fn halves_are_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Producer<u64>>();
    assert_send::<Consumer<u64>>();
    assert_send::<Producer<Box<String>>>();
    assert_send::<Consumer<Box<String>>>();
}
