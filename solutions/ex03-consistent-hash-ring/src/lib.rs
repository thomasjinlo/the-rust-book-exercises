//! Consistent hash ring.

use std::collections::BTreeSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub struct Ring {
    points_per_node: u32,
    // Sorted by (position, name). The name breaks ties, so two nodes that land on the same
    // position never depend on the order they were added.
    points: BTreeSet<(u64, String)>,
    nodes: BTreeSet<String>,
}

// `DefaultHasher::new()` uses fixed keys, so every ring computes the same value for the same input.
fn hash_of(value: &impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    value.hash(&mut h);
    h.finish()
}

impl Ring {
    pub fn new(points_per_node: u32) -> Ring {
        assert!(points_per_node >= 1);
        Ring {
            points_per_node,
            points: BTreeSet::new(),
            nodes: BTreeSet::new(),
        }
    }

    pub fn add_node(&mut self, name: &str) {
        if self.nodes.insert(name.to_owned()) {
            for i in 0..self.points_per_node {
                self.points.insert((hash_of(&(name, i)), name.to_owned()));
            }
        }
    }

    pub fn remove_node(&mut self, name: &str) {
        if self.nodes.remove(name) {
            self.points.retain(|(_, owner)| owner != name);
        }
    }

    pub fn get(&self, key: &[u8]) -> Option<&str> {
        let h = hash_of(&key);
        self.points
            .range((h, String::new())..)
            .next()
            .or_else(|| self.points.iter().next())
            .map(|(_, owner)| owner.as_str())
    }
}
