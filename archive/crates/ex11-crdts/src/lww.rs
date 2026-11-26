#![allow(unused_variables)]

use crate::{Crdt, ReplicaId};

/// Last-writer-wins register. The write with the greatest `(timestamp, replica)` wins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LwwRegister<T: Clone + PartialEq> {
    timestamp: u64,
    replica: ReplicaId,
    value: T,
}

impl<T: Clone + PartialEq> LwwRegister<T> {
    /// A register holding `value`, written at `(timestamp, replica)`.
    pub fn new(value: T, timestamp: u64, replica: ReplicaId) -> Self {
        todo!()
    }

    /// Applies the write only if `(timestamp, replica)` is strictly greater than
    /// the stored pair, comparing timestamp first, then replica id.
    /// An identical pair with a different value is ignored.
    pub fn set(&mut self, value: T, timestamp: u64, replica: ReplicaId) {
        todo!()
    }

    /// The current value.
    pub fn get(&self) -> &T {
        todo!()
    }
}

impl<T: Clone + PartialEq> Crdt for LwwRegister<T> {
    /// Keeps the greater `(timestamp, replica)` pair with its value.
    /// On an equal pair `self` is kept, so both sides must hold the same value
    /// for equal pairs (true whenever each replica id writes a timestamp once).
    fn merge(&mut self, other: &Self) {
        todo!()
    }
}
