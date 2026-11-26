use ex03_consistent_hash_ring::Ring;
use std::collections::HashMap;

fn names(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("node-{i}")).collect()
}

fn keys(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("key-{i}")).collect()
}

fn ring_of(points: u32, members: &[String]) -> Ring {
    let mut r = Ring::new(points);
    for m in members {
        r.add_node(m);
    }
    r
}

fn owners(r: &Ring, keys: &[String]) -> Vec<String> {
    keys.iter()
        .map(|k| r.get(k.as_bytes()).expect("non-empty ring").to_owned())
        .collect()
}

#[test]
fn an_empty_ring_has_no_owner() {
    let mut r = Ring::new(4);
    assert_eq!(r.get(b"k"), None);
    r.add_node("a");
    r.remove_node("a");
    assert_eq!(r.get(b"k"), None);
}

#[test]
fn one_node_owns_every_key() {
    let r = ring_of(4, &["solo".to_owned()]);
    assert_eq!(r.get(b""), Some("solo"));
    assert_eq!(r.get(&[0xff, 0x00]), Some("solo"));
    for k in keys(500) {
        assert_eq!(r.get(k.as_bytes()), Some("solo"));
    }
}

#[test]
fn every_owner_is_a_current_node() {
    let members = names(7);
    let r = ring_of(16, &members);
    for k in keys(2_000) {
        let owner = r.get(k.as_bytes()).unwrap();
        assert!(members.iter().any(|m| m == owner), "{owner}");
    }
}

#[test]
fn adding_a_node_twice_or_removing_a_missing_one_changes_nothing() {
    let ks = keys(2_000);
    let mut r = ring_of(16, &names(4));
    let before = owners(&r, &ks);
    r.add_node("node-2");
    r.remove_node("missing");
    assert_eq!(owners(&r, &ks), before);
}

#[test]
fn the_order_of_calls_does_not_change_any_answer() {
    let ks = keys(5_000);
    let members = names(12);
    let mut reversed = members.clone();
    reversed.reverse();
    assert_eq!(
        owners(&ring_of(40, &members), &ks),
        owners(&ring_of(40, &reversed), &ks)
    );

    // Same twelve nodes, reached by adding three extra nodes and removing them again.
    let mut detour = ring_of(40, &names(15));
    for extra in ["node-12", "node-13", "node-14"] {
        detour.remove_node(extra);
    }
    assert_eq!(owners(&detour, &ks), owners(&ring_of(40, &members), &ks));
}

#[test]
fn removing_a_node_moves_only_its_keys() {
    let ks = keys(20_000);
    let mut r = ring_of(64, &names(10));
    let before = owners(&r, &ks);
    r.remove_node("node-3");
    let after = owners(&r, &ks);

    let mut moved = 0;
    for (b, a) in before.iter().zip(&after) {
        assert_ne!(a, "node-3");
        if b == "node-3" {
            moved += 1;
        } else {
            assert_eq!(a, b, "a key the removed node did not own changed owner");
        }
    }
    assert!(
        moved > 0,
        "node-3 owned no keys, so this test proves nothing"
    );
}

#[test]
fn adding_a_node_moves_keys_only_to_the_new_node() {
    let ks = keys(20_000);
    let mut r = ring_of(64, &names(10));
    let before = owners(&r, &ks);
    r.add_node("node-10");
    let after = owners(&r, &ks);

    let mut moved = 0;
    for (b, a) in before.iter().zip(&after) {
        if b != a {
            moved += 1;
            assert_eq!(a, "node-10", "a key moved between two old nodes");
        }
    }
    assert!(
        moved > 0,
        "node-10 got no keys, so this test proves nothing"
    );
}

/// Same answers as a ring built from scratch for the current set of nodes.
#[test]
fn random_adds_and_removes_match_a_fresh_ring() {
    let mut rng = 0x2545_F491_4F6C_DD1Du64;
    let mut next = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    let ks = keys(300);
    let mut ring = Ring::new(8);
    let mut members: Vec<String> = Vec::new();
    for _ in 0..200 {
        let name = format!("node-{}", next() % 10);
        if next() % 2 == 0 {
            ring.add_node(&name);
            if !members.contains(&name) {
                members.push(name);
            }
        } else {
            ring.remove_node(&name);
            members.retain(|m| *m != name);
        }
        if members.is_empty() {
            assert_eq!(ring.get(b"key-0"), None);
        } else {
            assert_eq!(owners(&ring, &ks), owners(&ring_of(8, &members), &ks));
        }
    }
}

#[test]
fn keys_spread_evenly_and_a_new_node_takes_only_its_share() {
    let ks = keys(100_000);
    let mut r = ring_of(160, &names(10));
    let before = owners(&r, &ks);

    let mut counts: HashMap<&str, usize> = HashMap::new();
    for o in &before {
        *counts.entry(o).or_default() += 1;
    }
    assert_eq!(counts.len(), 10, "some node owns no keys");
    // An even split is 10,000 per node. The reference solution gives 9,304 to 10,337.
    // One point per node gives 836 to 26,446.
    for (node, c) in &counts {
        assert!((6_000..=14_500).contains(c), "{node} owns {c} keys");
    }

    r.add_node("node-10");
    let moved = before
        .iter()
        .zip(owners(&r, &ks))
        .filter(|(b, a)| *b != a)
        .count();
    // A fair share is 9.1%. The reference solution moves 8.4%.
    // A ring that rehashes every key moves about 91%.
    let frac = moved as f64 / ks.len() as f64;
    assert!((0.06..=0.13).contains(&frac), "moved {frac:.4}");
}
