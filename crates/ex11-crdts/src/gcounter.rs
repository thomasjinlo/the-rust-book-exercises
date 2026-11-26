#![allow(unused_variables)]

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
        todo!("raise the slot of `id`")
    }

    /// Sum of all slots, saturating at `u64::MAX`.
    pub fn value(&self) -> u64 {
        todo!("saturating sum of the slots")
    }
}

impl Crdt for GCounter {
    /// Pointwise maximum of the slots.
    fn merge(&mut self, other: &Self) {
        todo!("pointwise max")
    }
}
