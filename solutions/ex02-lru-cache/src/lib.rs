//! Fixed-capacity least-recently-used cache.
//!
//! Nodes live in a `Vec` and link to each other by index, most recent first.
//! A full cache reuses the least recent node's slot for the new entry.

use std::collections::HashMap;
use std::hash::Hash;

/// Index value meaning "no node".
const NIL: usize = usize::MAX;

struct Node<K, V> {
    key: K,
    value: V,
    prev: usize,
    next: usize,
}

/// A cache that holds at most `capacity` entries and evicts the least recently used one.
pub struct LruCache<K, V> {
    map: HashMap<K, usize>,
    nodes: Vec<Node<K, V>>,
    /// Most recently used node, or `NIL` when empty.
    head: usize,
    /// Least recently used node, or `NIL` when empty.
    tail: usize,
    capacity: usize,
}

impl<K: Hash + Eq + Clone, V> LruCache<K, V> {
    /// Creates an empty cache. `capacity` is at least 1.
    pub fn new(capacity: usize) -> Self {
        Self {
            map: HashMap::with_capacity(capacity),
            nodes: Vec::with_capacity(capacity),
            head: NIL,
            tail: NIL,
            capacity,
        }
    }

    /// Stores `value` under `key` and makes it the most recently used entry.
    /// When a new key arrives and the cache is full, the least recently used entry is removed.
    pub fn put(&mut self, key: K, value: V) {
        if let Some(&idx) = self.map.get(&key) {
            self.nodes[idx].value = value;
            self.move_to_front(idx);
            return;
        }
        if self.nodes.len() < self.capacity {
            self.nodes.push(Node {
                key: key.clone(),
                value,
                prev: NIL,
                next: NIL,
            });
            let idx = self.nodes.len() - 1;
            self.push_front(idx);
            self.map.insert(key, idx);
            return;
        }
        // Full: the least recent node's slot takes the new entry.
        let idx = self.tail;
        self.unlink(idx);
        let old_key = std::mem::replace(&mut self.nodes[idx].key, key.clone());
        self.map.remove(&old_key);
        self.nodes[idx].value = value;
        self.push_front(idx);
        self.map.insert(key, idx);
    }

    /// Returns the value for `key` and makes it the most recently used entry.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        let idx = *self.map.get(key)?;
        self.move_to_front(idx);
        Some(&self.nodes[idx].value)
    }

    fn unlink(&mut self, idx: usize) {
        let (prev, next) = (self.nodes[idx].prev, self.nodes[idx].next);
        if prev == NIL {
            self.head = next;
        } else {
            self.nodes[prev].next = next;
        }
        if next == NIL {
            self.tail = prev;
        } else {
            self.nodes[next].prev = prev;
        }
    }

    fn push_front(&mut self, idx: usize) {
        self.nodes[idx].prev = NIL;
        self.nodes[idx].next = self.head;
        if self.head == NIL {
            self.tail = idx;
        } else {
            self.nodes[self.head].prev = idx;
        }
        self.head = idx;
    }

    fn move_to_front(&mut self, idx: usize) {
        if self.head != idx {
            self.unlink(idx);
            self.push_front(idx);
        }
    }
}
