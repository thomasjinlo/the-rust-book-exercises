use ex03_consistent_hash_ring::{Ring, ZeroPoints};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::time::Duration;

/// Runs `f` on a thread and fails after 60 s instead of hanging the test run.
/// A correct run takes well under a second.
fn watchdog<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)));
    });
    match rx.recv_timeout(Duration::from_secs(60)) {
        Ok(Ok(v)) => v,
        Ok(Err(p)) => std::panic::resume_unwind(p),
        Err(_) => panic!("test exceeded 60 s"),
    }
}

fn names(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("node-{i}")).collect()
}

fn ring_of(points: u32, members: &[String]) -> Ring {
    let mut r = Ring::new(points).unwrap();
    for m in members {
        assert!(r.add_node(m));
    }
    r
}

fn keys(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("key-{i}")).collect()
}

fn owners(r: &Ring, keys: &[String]) -> Vec<String> {
    keys.iter()
        .map(|k| r.get(k.as_bytes()).expect("non-empty ring").to_owned())
        .collect()
}

/// Seeded xorshift64 so shuffles are the same on every run.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            v.swap(i, (self.next() % (i as u64 + 1)) as usize);
        }
    }
}

#[test]
fn zero_points_per_node_is_rejected() {
    assert_eq!(Ring::new(0).unwrap_err(), ZeroPoints);
}

#[test]
fn an_empty_ring_has_no_owner_and_no_replicas() {
    watchdog(|| {
        let r = Ring::new(4).unwrap();
        assert_eq!(r.node_count(), 0);
        assert_eq!(r.get(b"k"), None);
        assert!(r.get_replicas(b"k", 3).is_empty());
    });
}

#[test]
fn add_and_remove_report_whether_membership_changed() {
    watchdog(|| {
        let mut r = Ring::new(8).unwrap();
        assert!(r.add_node("a"));
        assert!(!r.add_node("a"));
        assert_eq!(r.node_count(), 1);
        assert!(!r.remove_node("missing"));
        assert_eq!(r.node_count(), 1);
        assert!(r.remove_node("a"));
        assert!(!r.remove_node("a"));
        assert_eq!(r.node_count(), 0);
        assert_eq!(r.get(b"k"), None);
    });
}

#[test]
fn a_single_node_owns_every_key() {
    watchdog(|| {
        let r = ring_of(4, &names(1));
        for k in keys(500) {
            assert_eq!(r.get(k.as_bytes()), Some("node-0"));
        }
        assert_eq!(r.get(b""), Some("node-0"));
        assert_eq!(r.get(&[0xff, 0xfe, 0x00, 0x80]), Some("node-0"));
    });
}

#[test]
fn every_owner_is_a_current_node() {
    watchdog(|| {
        let members = names(7);
        let r = ring_of(16, &members);
        let valid: HashSet<&str> = members.iter().map(String::as_str).collect();
        for k in keys(2_000) {
            assert!(valid.contains(r.get(k.as_bytes()).unwrap()));
        }
    });
}

#[test]
fn lookups_repeat_exactly() {
    watchdog(|| {
        let r = ring_of(32, &names(5));
        let ks = keys(1_000);
        assert_eq!(owners(&r, &ks), owners(&r, &ks));
    });
}

#[test]
fn insertion_order_does_not_change_any_answer() {
    watchdog(|| {
        let members = names(12);
        let ks = keys(5_000);
        let forward = ring_of(40, &members);
        let mut reversed_members = members.clone();
        reversed_members.reverse();
        let reversed = ring_of(40, &reversed_members);
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
        let mut shuffled_members = members.clone();
        rng.shuffle(&mut shuffled_members);
        let shuffled = ring_of(40, &shuffled_members);

        let expected = owners(&forward, &ks);
        assert_eq!(owners(&reversed, &ks), expected);
        assert_eq!(owners(&shuffled, &ks), expected);
        for k in ks.iter().take(300) {
            let want = forward.get_replicas(k.as_bytes(), 3);
            assert_eq!(reversed.get_replicas(k.as_bytes(), 3), want);
            assert_eq!(shuffled.get_replicas(k.as_bytes(), 3), want);
        }
    });
}

#[test]
fn a_ring_that_reached_the_same_members_by_other_steps_answers_the_same() {
    watchdog(|| {
        let members = names(8);
        let ks = keys(5_000);
        let direct = ring_of(40, &members);

        let mut detour = Ring::new(40).unwrap();
        for m in names(11) {
            detour.add_node(&m);
        }
        for extra in ["node-8", "node-9", "node-10"] {
            assert!(detour.remove_node(extra));
        }
        assert_eq!(detour.node_count(), 8);
        assert_eq!(owners(&detour, &ks), owners(&direct, &ks));
    });
}

#[test]
fn replicas_are_distinct_and_start_with_the_owner() {
    watchdog(|| {
        // One point per node makes neighbouring points belong to different nodes
        // only by chance, so a repeated node is likely if duplicates are not skipped.
        for points in [1, 3, 64] {
            let r = ring_of(points, &names(6));
            for k in keys(500) {
                let reps = r.get_replicas(k.as_bytes(), 4);
                assert_eq!(reps.len(), 4);
                assert_eq!(reps.iter().collect::<HashSet<_>>().len(), 4, "{reps:?}");
                assert_eq!(Some(reps[0]), r.get(k.as_bytes()));
            }
        }
    });
}

#[test]
fn replica_count_is_capped_by_membership_and_zero_asks_for_none() {
    watchdog(|| {
        let r = ring_of(8, &names(3));
        for k in keys(100) {
            assert!(r.get_replicas(k.as_bytes(), 0).is_empty());
            assert_eq!(r.get_replicas(k.as_bytes(), 1).len(), 1);
            assert_eq!(r.get_replicas(k.as_bytes(), 3).len(), 3);
            let all: HashSet<&str> = r.get_replicas(k.as_bytes(), 10).into_iter().collect();
            assert_eq!(all.len(), 3);
        }
    });
}

#[test]
fn removing_a_node_moves_only_its_keys_to_surviving_nodes() {
    watchdog(|| {
        let members = names(10);
        let ks = keys(20_000);
        let mut r = ring_of(64, &members);
        let before = owners(&r, &ks);
        assert!(r.remove_node("node-3"));
        let after = owners(&r, &ks);

        let mut moved = 0;
        for (b, a) in before.iter().zip(&after) {
            assert_ne!(a, "node-3");
            if b == "node-3" {
                moved += 1;
            } else {
                assert_eq!(a, b, "a key not owned by the removed node changed owner");
            }
        }
        assert!(moved > 0, "node-3 owned no keys; the test proves nothing");
    });
}

#[test]
fn removed_nodes_never_appear_in_replicas_and_re_adding_restores_answers() {
    watchdog(|| {
        let members = names(6);
        let ks = keys(1_000);
        let mut r = ring_of(32, &members);
        let before = owners(&r, &ks);
        r.remove_node("node-2");
        for k in &ks {
            assert!(!r.get_replicas(k.as_bytes(), 5).contains(&"node-2"));
        }
        r.add_node("node-2");
        assert_eq!(owners(&r, &ks), before);
    });
}

#[test]
fn adding_a_node_moves_a_bounded_share_of_keys_only_to_the_new_node() {
    watchdog(|| {
        let ks = keys(100_000);
        let mut r = ring_of(160, &names(10));
        let before = owners(&r, &ks);
        assert!(r.add_node("node-10"));
        let after = owners(&r, &ks);

        let mut moved = 0usize;
        for (b, a) in before.iter().zip(&after) {
            if b != a {
                moved += 1;
                assert_eq!(a, "node-10", "a key moved between two old nodes");
            }
        }
        // A fair share of 11 nodes is 9.09%.
        let frac = moved as f64 / ks.len() as f64;
        assert!((0.06..=0.13).contains(&frac), "moved fraction {frac:.4}");
    });
}

#[test]
fn ten_nodes_share_100_000_keys_without_gross_imbalance() {
    watchdog(|| {
        let r = ring_of(160, &names(10));
        let mut counts: HashMap<String, usize> = HashMap::new();
        for k in keys(100_000) {
            *counts
                .entry(r.get(k.as_bytes()).unwrap().to_owned())
                .or_default() += 1;
        }
        assert_eq!(counts.len(), 10, "some node owns no keys at all");
        for (node, c) in &counts {
            assert!(
                (6_000..=14_500).contains(c),
                "{node} owns {c} of 100000 keys; an even split is 10000"
            );
        }
    });
}
