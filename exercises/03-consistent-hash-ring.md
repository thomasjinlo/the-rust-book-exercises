# Exercise 03: Consistent hash ring with virtual nodes

**Time:** ~2.5 h | **Area:** distributed systems | **Crate:** `crates/ex03-consistent-hash-ring`
**Builds on:** none (standalone)

## Read first
- [Ch. 5 Structs](https://doc.rust-lang.org/book/ch05-00-structs.html)
- [Ch. 8 Collections](https://doc.rust-lang.org/book/ch08-00-common-collections.html)
- [Ch. 10 Generics, traits, lifetimes](https://doc.rust-lang.org/book/ch10-00-generics.html) (the borrowed `&str` returns)
- [Ch. 11 Testing](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Ch. 13.2 Iterators](https://doc.rust-lang.org/book/ch13-02-iterators.html) (`chain`, `map`)
- [`BTreeMap`](https://doc.rust-lang.org/std/collections/struct.BTreeMap.html), especially `range`
- [Consistent hashing](https://en.wikipedia.org/wiki/Consistent_hashing)

## What you are building
Sharded caches and partitioned stores map each key to a node. When a node joins or leaves, a plain `hash(key) % n` scheme reshuffles almost every key. A consistent hash ring reshuffles only the keys next to the changed node. You build a ring with virtual nodes, a replica walk for replication, and tests that measure balance and movement.

## Concepts
The ring is the number line of all `u64` values bent into a circle: after the largest value comes 0.

1. Hash each node name to a **position** on the circle.
2. Hash each key to a position.
3. The key belongs to the first node position at or after the key's position, moving clockwise. Past the top, wrap to the lowest position.

Worked example on a circle of 0..99 (then back to 0). Nodes A, B, C sit at 10, 40, 70.

```
 0         10(A)     40(B)     70(C)           99→0
 |----------A---------B---------C--------------|
```

| Key position | First node position at or after it | Owner |
|---|---|---|
| 5 | 10 | A |
| 45 | 70 | C |
| 90 | none above, wrap to 10 | A |
| 40 | 40 (exact match counts) | B |

Node D joins at 55. Only keys in 41..=55 now find D first. Key 45 moves from C to D. Keys 5 and 90 keep owner A. Every other node keeps all its keys. With `hash % n`, adding a fourth node would move about 3 of 4 keys.

One position per node gives uneven arcs. Here B owns 11..=40 (30 values) and C owns 41..=70 (30), but A owns 71..=99 and 0..=10 (40), and with other positions the split can be far worse. Real rings give each node many **virtual nodes** (vnodes), positions derived from `name#0`, `name#1`, and so on. Many small arcs average out to near-equal shares.

A replica walk continues clockwise from the key's position and collects distinct nodes. It answers "which nodes also store this key".

## Starting point
- Edit: `crates/ex03-consistent-hash-ring/src/lib.rs`. Replace each `todo!()` and change private fields freely.
- Do not edit: `fnv1a`, `fmix64`, `ring_hash`, `ZeroVnodes`, the public signatures, or `tests/ring.rs`.
- Run: `cd crates/ex03-consistent-hash-ring && cargo test`.
- On the unmodified starter, `cargo test` finishes in under 2 s: 1 test passes, 13 fail.
- A `BTreeMap<u64, String>` from position to node name is enough. Collisions are out of scope (see R2).

## Requirements
Hash definitions (provided):
- `fnv1a(bytes)` is 64-bit FNV-1a: start with offset `0xcbf29ce484222325`; for each byte, XOR it into the hash, then multiply by prime `0x100000001b3` with wrapping.
- `ring_hash(bytes) = fmix64(fnv1a(bytes))`, where `fmix64` is the MurmurHash3 64-bit finalizer. Plain FNV-1a changes mostly high bits when the last input bytes change, so vnodes named `name#0`..`name#159` cluster. With 10 nodes x 160 vnodes the measured load spread is 0.41x to 1.85x the mean with plain FNV-1a and 0.88x to 1.13x with `ring_hash`.

Requirements:
1. `Ring::new(v)` returns `Err(ZeroVnodes)` for `v == 0`, otherwise an empty ring.
2. Virtual node `i` (0-based, `0..v`) of node `name` has position `ring_hash(format!("{name}#{i}").as_bytes())`. The tests never produce two equal positions, so you need no collision rule.
3. `get(key)` returns the owner of the smallest position `>= ring_hash(key)`. If none exists, it returns the owner of the smallest position on the ring. An empty ring returns `None`.
4. A key whose hash equals a position exactly belongs to that position.
5. `add_node(name)` returns `true` if the node was added and `false` with no change if `name` already exists. `remove_node(name)` returns `true` if the node existed and `false` otherwise.
6. `node_count()` is the number of nodes. `vnode_count()` is `node_count() * v`.
7. `get_replicas(key, n)` walks clockwise from the key's position and returns up to `n` distinct node names in first-seen order. The first element equals `get(key)`. The result has `min(n, node_count())` elements, and is empty when `n == 0` or the ring is empty.
8. Balance: 10 nodes `node-0`..`node-9`, 160 vnodes each, keys `key-0`..`key-99999`. Each node's key count is between 0.65x and 1.35x of 10,000.
9. Minimal movement on add: after adding an 11th node to that ring, between 5% and 14% of the 100,000 keys change owner, and every moved key moves to the new node.
10. After `remove_node(x)`, no key is owned by `x`, and every key not previously owned by `x` keeps its owner.

## Common mistakes
- Using `range(h + 1..)` or `range(..=h)`. A key hashing exactly to a position belongs to it, so the lookup range is `h..`.
- Forgetting the wrap. `range(h..)` is empty for keys above the highest position, so fall back to the first entry of the map.
- In `get_replicas`, returning the same node twice because its vnodes are adjacent. Skip names already collected.
- In `get_replicas`, chaining `range(..=h)` for the wrapped part. That visits position `h` twice. Use `range(..h)`.
- Not removing vnode positions in `remove_node`. The node name leaves `nodes`, but its positions still own keys.
- Hashing the key differently from vnode names, for example hashing the `String` with `DefaultHasher`. Both must use `ring_hash` on bytes, or the pinned test fails.

## Milestones
| Stage | Do | Requirements | Tests green at the end | Run |
|---|---|---|---|---|
| 1 | Struct, `new`, `add_node`, `remove_node`, counts | R1, R5, R6 | `zero_vnodes_is_rejected`, `add_and_remove_report_membership_changes`, `vnode_count_is_nodes_times_vnodes_without_collisions` | `cargo test zero_vnodes`, `cargo test add_and_remove`, `cargo test vnode_count` |
| 2 | `get` with `range` and wraparound | R2, R3, R4 | `single_node_owns_every_key`, `pinned_owners_for_three_nodes_four_vnodes`, `key_above_highest_position_wraps_to_lowest`, `key_hashing_exactly_to_a_position_belongs_to_that_position` | `cargo test single_node`, `cargo test pinned`, `cargo test key_` |
| 3 | Removal effects | R10 | `removed_node_owns_nothing_and_others_keep_their_keys` | `cargo test removed_node` |
| 4 | Distribution at scale | R8, R9 | `balance_ten_nodes_160_vnodes`, `adding_a_node_moves_keys_only_to_the_new_node` | `cargo test balance`, `cargo test adding_a_node` |
| 5 | `get_replicas` | R7 | `empty_ring_returns_nothing`, `replicas_are_distinct_and_start_with_the_owner`, `replicas_are_capped_by_node_count_and_zero_is_empty` | `cargo test replicas`, `cargo test empty_ring` |

`provided_hashes_match_reference_values` passes from the start.

## Given tests
All in `tests/ring.rs`, 14 tests.

| Test | Requirement |
|---|---|
| `provided_hashes_match_reference_values` | hash definitions (passes on the starter) |
| `zero_vnodes_is_rejected` | R1 |
| `empty_ring_returns_nothing` | R3, R7 |
| `add_and_remove_report_membership_changes` | R5, R6 |
| `single_node_owns_every_key` | R3 |
| `vnode_count_is_nodes_times_vnodes_without_collisions` | R6 |
| `pinned_owners_for_three_nodes_four_vnodes` | R2, R3: 10 pinned key owners |
| `key_above_highest_position_wraps_to_lowest` | R3 wraparound |
| `key_hashing_exactly_to_a_position_belongs_to_that_position` | R4 |
| `removed_node_owns_nothing_and_others_keep_their_keys` | R10 |
| `balance_ten_nodes_160_vnodes` | R8 |
| `adding_a_node_moves_keys_only_to_the_new_node` | R9 |
| `replicas_are_distinct_and_start_with_the_owner` | R7 |
| `replicas_are_capped_by_node_count_and_zero_is_empty` | R7 |

## Acceptance
```
cd crates/ex03-consistent-hash-ring
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```
Pass condition: all three exit 0 and `cargo test` reports `14 passed; 0 failed`. The reference solution runs the suite in about 0.3 s in debug mode. Each test is guarded by a 60 s watchdog.

## Extra tests to write
1. Two nodes with 1 vnode each, keys named `<node>#0` and keys hashing just below and above each position (search `probe-N` keys). Catches an off-by-one in the `range` bound.
2. `get_replicas(key, 2)` on a ring with 1 vnode per node, for many keys. Catches a walk that returns the same node twice or skips the wrap.
3. Add the same node 100 times, then `vnode_count()` equals the count after one add. Catches double insertion.
4. Remove the last node, then `get` returns `None` and `vnode_count()` is 0. Catches leaked positions.
5. Add `a`, `b`, remove `b`, add `b` again. Every answer equals the ring before the removal. Catches state left behind by removal.
6. Keys that are empty or contain non-UTF-8 bytes. Catches an accidental `str` conversion.
7. Print the load spread for 8, 16 and 160 vnodes on 10 nodes and record the numbers in your notes. Shows how vnode count controls balance.

## Stretch (optional)
1. Collision rule: two vnodes can share a position. Make the lexicographically smaller node name own it, keep the other claimant so `remove_node` can hand the position back, and add a hasher parameter to force collisions in tests.
2. Weighted nodes: `add_weighted(name, weight)` gives `weight * v` vnodes. Check that load is proportional to weight within 15%.
3. Bounded-load hashing (https://arxiv.org/abs/1608.01350): cap each node at `ceil(1.25 * average)` keys. When the owner is full, take the next distinct node clockwise.
