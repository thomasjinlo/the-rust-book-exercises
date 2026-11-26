use std::collections::{BTreeMap, BTreeSet};

use crate::{Crdt, ReplicaId};

/// Unique id of one `add`: the adding replica and its per-replica counter.
type Tag = (ReplicaId, u64);

/// Observed-remove set with add-wins semantics.
///
/// State: the live tags per element, the set of removed tags, and the highest
/// counter issued per replica. Merged state is
/// `(live_a ∪ live_b) \ (tombstones_a ∪ tombstones_b)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrSet<T> {
    live: BTreeMap<T, BTreeSet<Tag>>,
    tombstones: BTreeSet<Tag>,
    clock: BTreeMap<ReplicaId, u64>,
}

impl<T: Ord + Clone> OrSet<T> {
    /// An empty set.
    pub fn new() -> Self {
        Self {
            live: BTreeMap::new(),
            tombstones: BTreeSet::new(),
            clock: BTreeMap::new(),
        }
    }

    /// Adds `elem` under a fresh tag `(id, n)`, where `n` is one more than the
    /// last counter this set issued for `id`. Adding a present element adds a second tag.
    pub fn add(&mut self, elem: T, id: ReplicaId) {
        let n = self.clock.entry(id).or_insert(0);
        *n += 1;
        self.live.entry(elem).or_default().insert((id, *n));
    }

    /// Removes the tags of `elem` that this set holds now. Tags added elsewhere
    /// and not yet merged survive. Removing an absent element changes nothing.
    pub fn remove(&mut self, elem: &T) {
        if let Some(tags) = self.live.remove(elem) {
            self.tombstones.extend(tags);
        }
    }

    /// True if `elem` has at least one live tag.
    pub fn contains(&self, elem: &T) -> bool {
        self.live.contains_key(elem)
    }

    /// Present elements in ascending order, without duplicates.
    pub fn elements(&self) -> Vec<T> {
        self.live.keys().cloned().collect()
    }
}

impl<T: Ord + Clone> Default for OrSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Ord + Clone> Crdt for OrSet<T> {
    fn merge(&mut self, other: &Self) {
        for (id, n) in &other.clock {
            let mine = self.clock.entry(*id).or_insert(0);
            *mine = (*mine).max(*n);
        }
        self.tombstones.extend(other.tombstones.iter().copied());
        for (elem, tags) in &other.live {
            self.live
                .entry(elem.clone())
                .or_default()
                .extend(tags.iter().copied());
        }
        let tombstones = &self.tombstones;
        self.live.retain(|_, tags| {
            tags.retain(|t| !tombstones.contains(t));
            !tags.is_empty()
        });
    }
}
