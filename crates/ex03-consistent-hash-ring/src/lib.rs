//! Consistent hash ring.
//!
//! Replace each `todo!()`. Keep the public signatures. Add any private fields and helpers you need.

/// Maps byte-string keys to named nodes.
pub struct Ring {
    // Replace this field with your own.
}

impl Ring {
    /// Creates an empty ring. `points_per_node` is at least 1.
    pub fn new(points_per_node: u32) -> Ring {
        let _ = points_per_node;
        todo!()
    }

    /// Adds `name`. Does nothing if it is already a node.
    pub fn add_node(&mut self, name: &str) {
        let _ = name;
        todo!()
    }

    /// Removes `name`. Does nothing if it is not a node.
    pub fn remove_node(&mut self, name: &str) {
        let _ = name;
        todo!()
    }

    /// Returns the node that owns `key`, or `None` on an empty ring.
    pub fn get(&self, key: &[u8]) -> Option<&str> {
        let _ = key;
        todo!()
    }
}
