//! The three merge laws, checked on states reached by random histories on
//! three replicas that exchange states at random points.

use ex11_crdts::{Crdt, GCounter, LwwRegister, OrSet, PnCounter};
use proptest::prelude::*;
use proptest::test_runner::RngSeed;

#[derive(Debug, Clone)]
enum Op {
    Inc(u8),
    Dec(u8),
    Set(u8, u8), // (value, timestamp); small ranges force timestamp ties
    Add(u8),
    Remove(u8),
    SyncFrom(usize),
}

fn op() -> impl Strategy<Value = (usize, Op)> {
    let op = prop_oneof![
        any::<u8>().prop_map(Op::Inc),
        any::<u8>().prop_map(Op::Dec),
        (0u8..4, 0u8..6).prop_map(|(v, t)| Op::Set(v, t)),
        (0u8..5).prop_map(Op::Add),
        (0u8..5).prop_map(Op::Remove),
        (0usize..3).prop_map(Op::SyncFrom),
    ];
    (0usize..3, op)
}

fn history() -> impl Strategy<Value = Vec<(usize, Op)>> {
    prop::collection::vec(op(), 0..40)
}

/// Replica `i` always acts under id `i`, as a deployment would.
trait Replica: Crdt + std::fmt::Debug {
    fn fresh(id: u32) -> Self;
    fn apply(&mut self, id: u32, op: &Op);
}

impl Replica for GCounter {
    fn fresh(_: u32) -> Self {
        GCounter::new()
    }
    fn apply(&mut self, id: u32, op: &Op) {
        if let Op::Inc(n) | Op::Dec(n) = op {
            self.increment(id, u64::from(*n));
        }
    }
}

impl Replica for PnCounter {
    fn fresh(_: u32) -> Self {
        PnCounter::new()
    }
    fn apply(&mut self, id: u32, op: &Op) {
        match op {
            Op::Inc(n) => self.increment(id, u64::from(*n)),
            Op::Dec(n) => self.decrement(id, u64::from(*n)),
            _ => {}
        }
    }
}

impl Replica for LwwRegister<u8> {
    fn fresh(id: u32) -> Self {
        LwwRegister::new(0, 0, id)
    }
    fn apply(&mut self, id: u32, op: &Op) {
        if let Op::Set(v, t) = op {
            self.set(*v, u64::from(*t), id);
        }
    }
}

impl Replica for OrSet<u8> {
    fn fresh(_: u32) -> Self {
        OrSet::new()
    }
    fn apply(&mut self, id: u32, op: &Op) {
        match op {
            Op::Add(e) => self.add(*e, id),
            Op::Remove(e) => self.remove(e),
            _ => {}
        }
    }
}

fn run<C: Replica>(history: &[(usize, Op)]) -> [C; 3] {
    let mut r: [C; 3] = [C::fresh(0), C::fresh(1), C::fresh(2)];
    for (i, op) in history {
        if let Op::SyncFrom(j) = op {
            let other = r[*j].clone();
            r[*i].merge(&other);
        } else {
            r[*i].apply(*i as u32, op);
        }
    }
    r
}

fn merged<C: Crdt>(a: &C, b: &C) -> C {
    let mut out = a.clone();
    out.merge(b);
    out
}

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: 256,
        failure_persistence: None,
        rng_seed: RngSeed::Fixed(0x11C4D7),
        ..ProptestConfig::default()
    }
}

macro_rules! laws {
    ($module:ident, $ty:ty) => {
        mod $module {
            use super::*;

            proptest! {
                #![proptest_config(config())]

                #[test]
                fn commutative(h in history()) {
                    let [a, b, _] = run::<$ty>(&h);
                    prop_assert_eq!(merged(&a, &b), merged(&b, &a));
                }

                #[test]
                fn associative(h in history()) {
                    let [a, b, c] = run::<$ty>(&h);
                    let left = merged(&merged(&a, &b), &c);
                    let right = merged(&a, &merged(&b, &c));
                    prop_assert_eq!(left, right);
                }

                #[test]
                fn idempotent(h in history()) {
                    let [a, b, _] = run::<$ty>(&h);
                    prop_assert_eq!(merged(&a, &a), a.clone());
                    let ab = merged(&a, &b);
                    prop_assert_eq!(merged(&ab, &b), ab);
                }
            }
        }
    };
}

laws!(gcounter, GCounter);
laws!(pncounter, PnCounter);
laws!(lww, LwwRegister<u8>);
laws!(orset, OrSet<u8>);

proptest! {
    #![proptest_config(config())]

    #[test]
    fn gcounter_merge_never_decreases_the_value(h in history()) {
        let [mut a, b, c] = run::<GCounter>(&h);
        let before = a.value();
        a.merge(&b);
        prop_assert!(a.value() >= before);
        prop_assert!(a.value() >= b.value());
        let mid = a.value();
        a.merge(&c);
        prop_assert!(a.value() >= mid);
    }

    #[test]
    fn orset_merge_result_is_independent_of_merge_order(h in history()) {
        let [a, b, c] = run::<OrSet<u8>>(&h);
        let orders = [[&a, &b, &c], [&a, &c, &b], [&b, &a, &c], [&b, &c, &a], [&c, &a, &b], [&c, &b, &a]];
        let results: Vec<OrSet<u8>> = orders
            .iter()
            .map(|o| merged(&merged(o[0], o[1]), o[2]))
            .collect();
        for r in &results[1..] {
            prop_assert_eq!(r, &results[0]);
        }
    }
}
