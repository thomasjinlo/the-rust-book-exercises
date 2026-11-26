//! Five replicas, 300 random operations, random pairwise merges (duplicated,
//! reordered and stale), then one anti-entropy round. All replicas must be equal.

use ex11_crdts::{Crdt, GCounter, LwwRegister, OrSet, PnCounter, all_pairs_sync};

const REPLICAS: usize = 5;
const OPS: usize = 300;
const SEEDS: u64 = 50;

/// SplitMix64: small, fast, and identical on every platform.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform enough for a test: modulo bias is irrelevant at these ranges.
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

#[derive(Clone, PartialEq, Debug)]
struct Node {
    hits: GCounter,
    balance: PnCounter,
    name: LwwRegister<u64>,
    tags: OrSet<u8>,
}

impl Node {
    fn new(id: u32) -> Self {
        Node {
            hits: GCounter::new(),
            balance: PnCounter::new(),
            name: LwwRegister::new(0, 0, id),
            tags: OrSet::new(),
        }
    }
}

impl Crdt for Node {
    fn merge(&mut self, other: &Self) {
        let before = self.hits.value();
        self.hits.merge(&other.hits);
        assert!(self.hits.value() >= before, "GCounter value decreased");
        self.balance.merge(&other.balance);
        self.name.merge(&other.name);
        self.tags.merge(&other.tags);
    }
}

/// What the run applied, so the converged state can be checked against it.
#[derive(Default)]
struct Expected {
    hits: u64,
    balance: i128,
    last_name: u64,
    added: [bool; 8],
    removed: [bool; 8],
}

fn run(seed: u64) {
    let mut rng = Rng(seed);
    let mut nodes: Vec<Node> = (0..REPLICAS as u32).map(Node::new).collect();
    let mut snapshots: Vec<Node> = Vec::new();
    let mut exp = Expected::default();

    for step in 1..=OPS {
        let i = rng.below(REPLICAS);
        let id = i as u32;
        match rng.below(5) {
            0 => {
                let by = rng.below(100) as u64;
                nodes[i].hits.increment(id, by);
                nodes[i].balance.increment(id, by);
                exp.hits += by;
                exp.balance += by as i128;
            }
            1 => {
                let by = rng.below(100) as u64;
                nodes[i].balance.decrement(id, by);
                exp.balance -= by as i128;
            }
            2 => {
                // `step` is unique, so the last set in the run has the greatest pair.
                nodes[i].name.set(step as u64, step as u64, id);
                exp.last_name = step as u64;
            }
            3 => {
                let e = rng.below(8);
                nodes[i].tags.add(e as u8, id);
                exp.added[e] = true;
            }
            _ => {
                let e = rng.below(8);
                nodes[i].tags.remove(&(e as u8));
                exp.removed[e] = true;
            }
        }

        if rng.below(4) == 0 {
            snapshots.push(nodes[rng.below(REPLICAS)].clone());
        }
        // Up to two deliveries per step; a snapshot may be delivered again later.
        for _ in 0..rng.below(3) {
            let to = rng.below(REPLICAS);
            let from = if !snapshots.is_empty() && rng.below(2) == 0 {
                snapshots[rng.below(snapshots.len())].clone()
            } else {
                nodes[rng.below(REPLICAS)].clone()
            };
            nodes[to].merge(&from);
            if rng.below(3) == 0 {
                nodes[to].merge(&from); // duplicate delivery
            }
        }
    }

    all_pairs_sync(&mut nodes);

    for (i, n) in nodes.iter().enumerate() {
        assert_eq!(n, &nodes[0], "seed {seed}: replica {i} diverged");
    }
    let n = &nodes[0];
    assert_eq!(n.hits.value(), exp.hits, "seed {seed}: GCounter");
    assert_eq!(n.balance.value(), exp.balance, "seed {seed}: PnCounter");
    assert_eq!(*n.name.get(), exp.last_name, "seed {seed}: LwwRegister");
    for e in 0..8usize {
        if !exp.added[e] {
            assert!(!n.tags.contains(&(e as u8)), "seed {seed}: {e} never added");
        }
        if exp.added[e] && !exp.removed[e] {
            assert!(
                n.tags.contains(&(e as u8)),
                "seed {seed}: {e} never removed"
            );
        }
    }
    let listed = n.tags.elements();
    assert!(
        listed.windows(2).all(|w| w[0] < w[1]),
        "seed {seed}: unsorted"
    );
}

#[test]
fn fifty_seeds_converge() {
    for seed in 0..SEEDS {
        run(seed);
    }
}

#[test]
fn all_pairs_sync_on_independent_writes_equals_the_join() {
    let mut nodes: Vec<Node> = (0..3u32).map(Node::new).collect();
    for (i, n) in nodes.iter_mut().enumerate() {
        n.hits.increment(i as u32, 10);
        n.tags.add(i as u8, i as u32);
    }
    all_pairs_sync(&mut nodes);
    assert_eq!(nodes[0], nodes[1]);
    assert_eq!(nodes[1], nodes[2]);
    assert_eq!(nodes[0].hits.value(), 30);
    assert_eq!(nodes[0].tags.elements(), vec![0, 1, 2]);
}
