use crate::{Crdt, GCounter, ReplicaId};

/// Counter that supports decrements: one `GCounter` of increments, one of decrements.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PnCounter {
    pos: GCounter,
    neg: GCounter,
}

impl PnCounter {
    /// An empty counter. `value()` is 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `by` to replica `id`'s increments. `by == 0` leaves the state unchanged.
    pub fn increment(&mut self, id: ReplicaId, by: u64) {
        self.pos.increment(id, by);
    }

    /// Adds `by` to replica `id`'s decrements. `by == 0` leaves the state unchanged.
    pub fn decrement(&mut self, id: ReplicaId, by: u64) {
        self.neg.increment(id, by);
    }

    /// Total increments minus total decrements.
    ///
    /// Each total saturates at `u64::MAX` before the subtraction, so the result
    /// always fits in an `i128`.
    pub fn value(&self) -> i128 {
        i128::from(self.pos.value()) - i128::from(self.neg.value())
    }
}

impl Crdt for PnCounter {
    fn merge(&mut self, other: &Self) {
        self.pos.merge(&other.pos);
        self.neg.merge(&other.neg);
    }
}
