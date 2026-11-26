#![allow(unused_variables)]

use std::marker::PhantomData;

use crate::{Crdt, ReplicaId};

/// Observed-remove set with add-wins semantics.
///
/// Design the state yourself and replace the placeholder field. Required:
/// `Debug`, `Clone` and `PartialEq` must compare the full state, so two
/// replicas that converged are `==`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrSet<T> {
    placeholder: PhantomData<T>,
}

impl<T: Ord + Clone> OrSet<T> {
    /// An empty set.
    pub fn new() -> Self {
        todo!()
    }

    /// Adds `elem` under a tag unique to this add. The tag is `(id, n)`, where
    /// `n` comes from a per-replica counter kept in the set.
    pub fn add(&mut self, elem: T, id: ReplicaId) {
        todo!()
    }

    /// Removes the tags of `elem` that this set holds now. Tags added elsewhere
    /// and not yet merged survive. Removing an absent element changes nothing,
    /// including the state compared by `==`.
    pub fn remove(&mut self, elem: &T) {
        todo!()
    }

    /// True if `elem` has at least one live tag.
    pub fn contains(&self, elem: &T) -> bool {
        todo!()
    }

    /// Present elements in ascending order, without duplicates.
    pub fn elements(&self) -> Vec<T> {
        todo!()
    }
}

impl<T: Ord + Clone> Default for OrSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Ord + Clone> Crdt for OrSet<T> {
    fn merge(&mut self, other: &Self) {
        todo!()
    }
}
