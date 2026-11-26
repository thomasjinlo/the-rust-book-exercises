use std::collections::BTreeMap;

use crate::{Crdt, ReplicaId};

/// Grow-only counter. Each replica owns one slot and only raises it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GCounter {
    slots: BTreeMap<ReplicaId, u64>,
}

impl GCounter {
    /// An empty counter. `value()` is 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `by` to the slot of replica `id`, saturating at `u64::MAX`.
    /// `by == 0` leaves the state unchanged (no empty slot is created).
    pub fn increment(&mut self, id: ReplicaId, by: u64) {
        if by == 0 {
            return;
        }
        let slot = self.slots.entry(id).or_insert(0);
        *slot = slot.saturating_add(by);
    }

    /// Sum of all slots, saturating at `u64::MAX`.
    pub fn value(&self) -> u64 {
        self.slots.values().fold(0, |acc, v| acc.saturating_add(*v))
    }
}

impl Crdt for GCounter {
    /// Pointwise maximum of the slots.
    fn merge(&mut self, other: &Self) {
        for (id, v) in &other.slots {
            let slot = self.slots.entry(*id).or_insert(0);
            *slot = (*slot).max(*v);
        }
    }
}
