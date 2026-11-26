//! Consistent hash ring with virtual nodes.
//!
//! Ring hash: `ring_hash(x) = fmix64(fnv1a(x))`. Virtual node `i` (0-based) of node
//! `name` sits at ring position `ring_hash(format!("{name}#{i}").as_bytes())`.
//! A key is owned by the first position `>=` `ring_hash(key)`, wrapping to the lowest position.
//! Position collisions are out of scope: the tests never produce one.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// 64-bit FNV-1a. Provided; do not edit.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h = FNV_OFFSET;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

/// MurmurHash3 64-bit finalizer. Provided; do not edit.
///
/// FNV-1a changes only the high bits when the last input bytes change, so vnodes named
/// `name#0`..`name#159` would form a few tight clusters. This step spreads them.
pub fn fmix64(mut h: u64) -> u64 {
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
    h ^= h >> 33;
    h = h.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    h ^ (h >> 33)
}

/// Hash used for vnode positions and keys: `fmix64(fnv1a(bytes))`. Provided; do not edit.
pub fn ring_hash(bytes: &[u8]) -> u64 {
    fmix64(fnv1a(bytes))
}

/// Returned by [`Ring::new`] when `vnodes_per_node` is 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZeroVnodes;

impl fmt::Display for ZeroVnodes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("vnodes_per_node must be at least 1")
    }
}

impl std::error::Error for ZeroVnodes {}

/// Consistent hash ring. You may change the private fields freely.
#[derive(Debug, Clone)]
pub struct Ring {
    vnodes_per_node: u32,
    /// Ring position -> owning node name.
    positions: BTreeMap<u64, String>,
    nodes: BTreeSet<String>,
}

impl Ring {
    /// Creates an empty ring.
    ///
    /// Errors: `ZeroVnodes` if `vnodes_per_node == 0`.
    pub fn new(vnodes_per_node: u32) -> Result<Ring, ZeroVnodes> {
        let _ = vnodes_per_node;
        todo!("reject 0 vnodes, build an empty ring")
    }

    /// Adds `name` with `vnodes_per_node` virtual nodes.
    ///
    /// Returns `true` if added, `false` (ring unchanged) if `name` is already present.
    pub fn add_node(&mut self, name: &str) -> bool {
        let _ = name;
        todo!()
    }

    /// Removes `name` and its vnodes.
    ///
    /// Returns `true` if removed, `false` if `name` was not present.
    pub fn remove_node(&mut self, name: &str) -> bool {
        let _ = name;
        todo!()
    }

    /// Owner of `key`, or `None` on an empty ring.
    pub fn get(&self, key: &[u8]) -> Option<&str> {
        let _ = key;
        todo!()
    }

    /// Up to `n` distinct nodes, walking clockwise from the key's position.
    /// The first element equals `get(key)`. Returns fewer than `n` when the ring
    /// has fewer nodes, and an empty `Vec` when `n == 0` or the ring is empty.
    pub fn get_replicas(&self, key: &[u8], n: usize) -> Vec<&str> {
        let _ = (key, n);
        todo!()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize {
        todo!()
    }

    /// Number of ring positions: `node_count() * vnodes_per_node`.
    pub fn vnode_count(&self) -> usize {
        todo!()
    }
}
