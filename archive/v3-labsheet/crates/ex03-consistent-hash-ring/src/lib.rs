//! Consistent hash ring.

use std::fmt;

/// Returned by [`Ring::new`] when `points_per_node` is 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZeroPoints;

impl fmt::Display for ZeroPoints {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("points_per_node must be at least 1")
    }
}

impl std::error::Error for ZeroPoints {}

/// Consistent hash ring. Add whatever private fields you need.
#[derive(Debug, Clone)]
pub struct Ring {}

impl Ring {
    /// Creates an empty ring. `Err(ZeroPoints)` if `points_per_node == 0`.
    pub fn new(points_per_node: u32) -> Result<Ring, ZeroPoints> {
        let _ = points_per_node;
        todo!()
    }

    /// Adds `name`. Returns `true` if it was added, `false` if it was already a member.
    pub fn add_node(&mut self, name: &str) -> bool {
        let _ = name;
        todo!()
    }

    /// Removes `name`. Returns `true` if it was a member, `false` otherwise.
    pub fn remove_node(&mut self, name: &str) -> bool {
        let _ = name;
        todo!()
    }

    /// Number of member nodes.
    pub fn node_count(&self) -> usize {
        todo!()
    }

    /// Owner of `key`, or `None` on an empty ring.
    pub fn get(&self, key: &[u8]) -> Option<&str> {
        let _ = key;
        todo!()
    }

    /// Up to `n` distinct nodes responsible for `key`, owner first.
    pub fn get_replicas(&self, key: &[u8], n: usize) -> Vec<&str> {
        let _ = (key, n);
        todo!()
    }
}
