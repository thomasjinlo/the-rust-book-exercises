//! Consistent hash ring.

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::hash::{Hash, Hasher};

/// Returned by [`Ring::new`] when `points_per_node` is 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZeroPoints;

impl fmt::Display for ZeroPoints {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("points_per_node must be at least 1")
    }
}

impl std::error::Error for ZeroPoints {}

#[derive(Debug, Clone)]
pub struct Ring {
    points_per_node: u32,
    /// Ring position -> owning node.
    points: BTreeMap<u64, String>,
    nodes: BTreeSet<String>,
}

/// `DefaultHasher::new()` is SipHash with fixed keys, so every ring in every
/// process computes the same value for the same input.
fn hash_of(value: &impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    value.hash(&mut h);
    h.finish()
}

impl Ring {
    pub fn new(points_per_node: u32) -> Result<Ring, ZeroPoints> {
        if points_per_node == 0 {
            return Err(ZeroPoints);
        }
        Ok(Ring {
            points_per_node,
            points: BTreeMap::new(),
            nodes: BTreeSet::new(),
        })
    }

    pub fn add_node(&mut self, name: &str) -> bool {
        if !self.nodes.insert(name.to_owned()) {
            return false;
        }
        for pos in self.positions(name) {
            self.points.insert(pos, name.to_owned());
        }
        true
    }

    pub fn remove_node(&mut self, name: &str) -> bool {
        if !self.nodes.remove(name) {
            return false;
        }
        for pos in self.positions(name) {
            self.points.remove(&pos);
        }
        true
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn get(&self, key: &[u8]) -> Option<&str> {
        let h = hash_of(&key);
        self.points
            .range(h..)
            .next()
            .or_else(|| self.points.iter().next())
            .map(|(_, owner)| owner.as_str())
    }

    pub fn get_replicas(&self, key: &[u8], n: usize) -> Vec<&str> {
        let want = n.min(self.nodes.len());
        let mut out: Vec<&str> = Vec::with_capacity(want);
        if want == 0 {
            return out;
        }
        let h = hash_of(&key);
        // The second range is the wrapped part; `..h` leaves out `h`, which the first range holds.
        let walk = self.points.range(h..).chain(self.points.range(..h));
        for (_, owner) in walk {
            if !out.contains(&owner.as_str()) {
                out.push(owner);
                if out.len() == want {
                    break;
                }
            }
        }
        out
    }

    fn positions(&self, name: &str) -> Vec<u64> {
        (0..self.points_per_node)
            .map(|i| hash_of(&(name, i)))
            .collect()
    }
}
