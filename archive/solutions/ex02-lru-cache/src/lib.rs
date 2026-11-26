//! O(1) LRU cache: a `Vec` slab of nodes linked by `usize` indices, a free
//! list that reuses vacated slots, and a `HashMap<K, usize>` from key to slot.
//!
//! No `unsafe`, no `Rc`, no `RefCell`.

use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;

/// Index value meaning "no node". Slab indices are always below this.
const NIL: usize = usize::MAX;

/// Returned by [`LruCache::new`] when `capacity == 0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZeroCapacity;

impl fmt::Display for ZeroCapacity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cache capacity must be at least 1")
    }
}

impl std::error::Error for ZeroCapacity {}

/// One list node. `prev` points toward the most recently used end, `next`
/// toward the least recently used end. `NIL` marks either end of the list.
struct Node<K, V> {
    key: K,
    value: V,
    prev: usize,
    next: usize,
}

/// A slab slot: a live node, or a link in the free list.
enum Slot<K, V> {
    Occupied(Node<K, V>),
    Vacant { next_free: usize },
}

/// Fixed-capacity cache that evicts the least recently used entry.
///
pub struct LruCache<K, V> {
    map: HashMap<K, usize>,
    slots: Vec<Slot<K, V>>,
    /// Most recently used slot, or `NIL` when empty.
    head: usize,
    /// Least recently used slot, or `NIL` when empty.
    tail: usize,
    /// First vacant slot, or `NIL` when none.
    free: usize,
    capacity: usize,
}

impl<K: Hash + Eq + Clone, V> LruCache<K, V> {
    /// Creates an empty cache holding at most `capacity` entries.
    ///
    /// Returns `Err(ZeroCapacity)` when `capacity == 0`.
    pub fn new(capacity: usize) -> Result<Self, ZeroCapacity> {
        if capacity == 0 {
            return Err(ZeroCapacity);
        }
        Ok(Self {
            map: HashMap::new(),
            slots: Vec::new(),
            head: NIL,
            tail: NIL,
            free: NIL,
            capacity,
        })
    }

    /// Returns the value for `key` and makes the entry most recently used.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        let idx = *self.map.get(key)?;
        self.move_to_front(idx);
        Some(&self.node(idx).value)
    }

    /// Like [`get`](Self::get) but returns a mutable reference.
    /// A hit makes the entry most recently used.
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        let idx = *self.map.get(key)?;
        self.move_to_front(idx);
        Some(&mut self.node_mut(idx).value)
    }

    /// Returns the value for `key` without changing recency.
    pub fn peek(&self, key: &K) -> Option<&V> {
        let idx = *self.map.get(key)?;
        Some(&self.node(idx).value)
    }

    /// Inserts or updates `key`; the entry becomes most recently used.
    ///
    /// - Existing key: the old value is dropped, the new one stored, returns `None`.
    /// - New key, cache full: evicts the least recently used entry first and
    ///   returns it as `Some((key, value))`.
    /// - New key, cache not full: returns `None`.
    pub fn put(&mut self, key: K, value: V) -> Option<(K, V)> {
        if let Some(&idx) = self.map.get(&key) {
            self.node_mut(idx).value = value;
            self.move_to_front(idx);
            return None;
        }
        // Evict before allocating so the slab never grows past `capacity`.
        let evicted = if self.map.len() == self.capacity {
            let node = self.take_node(self.tail);
            self.map.remove(&node.key);
            Some((node.key, node.value))
        } else {
            None
        };
        let idx = self.alloc(Node {
            key: key.clone(),
            value,
            prev: NIL,
            next: NIL,
        });
        self.push_front(idx);
        self.map.insert(key, idx);
        evicted
    }

    /// Removes `key` and returns its value. The vacated slot joins the free list.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let idx = self.map.remove(key)?;
        Some(self.take_node(idx).value)
    }

    /// Number of live entries.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Maximum number of live entries, as passed to `new`.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// True when `len() == 0`.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Number of slab slots currently allocated, live or vacant.
    /// Must never exceed `capacity()`. `clear` resets it to 0.
    pub fn slab_len(&self) -> usize {
        self.slots.len()
    }

    /// Iterates entries from most to least recently used. Does not change recency.
    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter {
            cache: self,
            next: self.head,
        }
    }

    /// Removes all entries, dropping every value exactly once. Capacity is unchanged.
    pub fn clear(&mut self) {
        self.map.clear();
        self.slots.clear();
        self.head = NIL;
        self.tail = NIL;
        self.free = NIL;
    }

    fn node(&self, idx: usize) -> &Node<K, V> {
        match &self.slots[idx] {
            Slot::Occupied(node) => node,
            Slot::Vacant { .. } => unreachable!("index {idx} is on the free list"),
        }
    }

    fn node_mut(&mut self, idx: usize) -> &mut Node<K, V> {
        match &mut self.slots[idx] {
            Slot::Occupied(node) => node,
            Slot::Vacant { .. } => unreachable!("index {idx} is on the free list"),
        }
    }

    /// Stores `node` in a vacant slot if one exists, else grows the slab.
    fn alloc(&mut self, node: Node<K, V>) -> usize {
        if self.free == NIL {
            self.slots.push(Slot::Occupied(node));
            return self.slots.len() - 1;
        }
        let idx = self.free;
        let old = std::mem::replace(&mut self.slots[idx], Slot::Occupied(node));
        match old {
            Slot::Vacant { next_free } => self.free = next_free,
            Slot::Occupied(_) => unreachable!("free list points at a live slot"),
        }
        idx
    }

    /// Unlinks slot `idx`, puts it on the free list and returns its node.
    /// The caller removes the key from `map`.
    fn take_node(&mut self, idx: usize) -> Node<K, V> {
        self.unlink(idx);
        let vacant = Slot::Vacant {
            next_free: self.free,
        };
        self.free = idx;
        match std::mem::replace(&mut self.slots[idx], vacant) {
            Slot::Occupied(node) => node,
            Slot::Vacant { .. } => unreachable!("index {idx} is already vacant"),
        }
    }

    fn unlink(&mut self, idx: usize) {
        let (prev, next) = {
            let node = self.node(idx);
            (node.prev, node.next)
        };
        if prev == NIL {
            self.head = next;
        } else {
            self.node_mut(prev).next = next;
        }
        if next == NIL {
            self.tail = prev;
        } else {
            self.node_mut(next).prev = prev;
        }
    }

    /// Links the unlinked node `idx` in as the new head.
    fn push_front(&mut self, idx: usize) {
        let old_head = self.head;
        let node = self.node_mut(idx);
        node.prev = NIL;
        node.next = old_head;
        if old_head == NIL {
            self.tail = idx;
        } else {
            self.node_mut(old_head).prev = idx;
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

/// Iterator returned by [`LruCache::iter`].
pub struct Iter<'a, K, V> {
    cache: &'a LruCache<K, V>,
    next: usize,
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        if self.next == NIL {
            return None;
        }
        match &self.cache.slots[self.next] {
            Slot::Occupied(node) => {
                self.next = node.next;
                Some((&node.key, &node.value))
            }
            Slot::Vacant { .. } => unreachable!("list links a vacant slot"),
        }
    }
}
