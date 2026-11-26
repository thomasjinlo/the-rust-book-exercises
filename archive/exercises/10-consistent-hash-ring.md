# Exercise 10: Consistent hash ring with virtual nodes

**Time:** ~2.5 h | **Area:** distributed systems | **Crate:** `crates/ex10-consistent-hash-ring`
**Builds on:** none (standalone)

## Read first
- Rust Book ch08, collections: https://doc.rust-lang.org/book/ch08-00-common-collections.html
- Rust Book ch10, generics, traits, lifetimes (the borrowed `&str` returns): https://doc.rust-lang.org/book/ch10-00-generics.html
- Rust Book ch11, testing: https://doc.rust-lang.org/book/ch11-00-testing.html
- `BTreeMap`, especially `range`: https://doc.rust-lang.org/std/collections/struct.BTreeMap.html
- Consistent hashing: https://en.wikipedia.org/wiki/Consistent_hashing
- FNV hash: http://www.isthe.com/chongo/tech/comp/fnv/ (alternative: https://en.wikipedia.org/wiki/Fowler%E2%80%93Noll%E2%80%93Vo_hash_function)

## What you are building
Sharded caches and partitioned stores must map each key to a node, and must not reshuffle most keys when a node joins or leaves. A consistent hash ring places each node at many pseudo-random positions (virtual nodes) on a 64-bit circle. A key belongs to the first position at or after its hash. Adding or removing a node moves only the keys next to that node's positions. You build the ring, a replica walk for replication, and tests that measure balance and movement.

## Starting point
- Edit: `crates/ex10-consistent-hash-ring/src/lib.rs`. Replace each `todo!()` and change private fields freely.
- Do not edit: `fnv1a`, `fmix64`, `ring_hash`, `ZeroVnodes`, the public signatures, or `tests/ring.rs`.
- Run: `cd crates/ex10-consistent-hash-ring && cargo test`.
- `Ring::with_hasher` is `#[doc(hidden)]`. Tests use it to inject a hash function and force collisions. `Ring::new` must delegate to it with `ring_hash`.
- On the unmodified starter, `cargo test` finishes in under 1 s: 1 test passes, 17 fail.

## Requirements
Hash definitions (provided):
- `fnv1a(bytes)` is 64-bit FNV-1a: offset `0xcbf29ce484222325`, prime `0x100000001b3`, over the bytes.
- `ring_hash(bytes) = fmix64(fnv1a(bytes))`, where `fmix64` is the MurmurHash3 64-bit finalizer. Plain FNV-1a gives vnodes named `name#0`..`name#159` only a few tight clusters, because it changes mostly high bits when the last input bytes change. With plain FNV-1a on this naming, 10 nodes x 160 vnodes measured a load spread of 0.41x to 1.85x the mean. With `ring_hash` it is 0.88x to 1.13x.

Requirements:
1. `Ring::new(v)` returns `Err(ZeroVnodes)` for `v == 0`, otherwise an empty ring.
2. Virtual node `i` (0-based, `0..v`) of node `name` has position `h(format!("{name}#{i}").as_bytes())`, where `h` is the ring's hash function (`ring_hash` for `Ring::new`).
3. `get(key)` returns the owner of the smallest position `>= h(key)`. If none exists, it returns the owner of the smallest position on the ring. An empty ring returns `None`.
4. A key whose hash equals a position exactly belongs to that position.
5. If several vnodes share a position, the node with the lexicographically smaller name (byte order of `str`) owns it. The others lose that vnode silently. When the owner is removed, the position passes to the smallest remaining claimant. Vnodes of one node that collide with each other count once.
6. `add_node(name)` returns `true` if the node was added and `false` with no change if `name` already exists. `remove_node(name)` returns `true` if the node existed and `false` otherwise.
7. `node_count()` is the number of nodes. `vnode_count()` is the number of occupied positions, so colliding vnodes count once.
8. `get_replicas(key, n)` walks clockwise from the key's position and returns up to `n` distinct node names in first-seen order. The first element equals `get(key)`. The result has `min(n, node_count())` elements, and is empty when `n == 0` or the ring is empty.
9. Answers depend only on the set of nodes, not on the order of `add_node`/`remove_node` calls that produced it. Adding then removing a node restores every answer.
10. Balance: 10 nodes `node-0`..`node-9`, 160 vnodes each, keys `key-0`..`key-99999`. Each node's key count is between 0.65x and 1.35x of 10,000.
11. Minimal movement on add: after adding an 11th node to that ring, between 5% and 14% of the 100,000 keys change owner, and every moved key moves to the new node.
12. Minimal movement on remove: after removing a node, keys it did not own keep their owner. Each key it owned moves to the node that was second in that key's `get_replicas(key, 2)` before the removal.

## Given tests
All in `tests/ring.rs`, 18 tests.

| Test | Requirement |
|---|---|
| `provided_hashes_match_reference_values` | hash definitions (passes on the starter) |
| `zero_vnodes_is_rejected` | R1 |
| `empty_ring_returns_nothing` | R3, R8 |
| `add_and_remove_report_membership_changes` | R6, R7 |
| `single_node_owns_every_key` | R3 |
| `vnode_count_is_nodes_times_vnodes_without_collisions` | R7 |
| `pinned_owners_for_three_nodes_four_vnodes` | R2, R3: 10 pinned key owners |
| `key_above_highest_position_wraps_to_lowest` | R3 wraparound |
| `key_hashing_exactly_to_a_position_belongs_to_that_position` | R4 |
| `collision_goes_to_lexicographically_smaller_name` | R5, insertion order both ways |
| `removing_collision_winner_gives_position_to_the_other_node` | R5 |
| `balance_ten_nodes_160_vnodes` | R10 |
| `adding_a_node_moves_keys_only_to_the_new_node` | R11 |
| `removing_a_node_moves_only_its_keys_to_the_next_distinct_neighbor` | R12 |
| `replicas_are_distinct_and_start_with_the_owner` | R8 |
| `replicas_are_capped_by_node_count_and_zero_is_empty` | R8 |
| `insertion_order_does_not_change_any_answer` | R9, proptest over permutations of 8 nodes, seed fixed |
| `add_then_remove_restores_every_answer` | R9, proptest |

## Acceptance
```
cd crates/ex10-consistent-hash-ring
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```
Pass condition: all three exit 0 and `cargo test` reports `18 passed; 0 failed`. The reference solution runs the suite in about 0.3 s in debug mode. Each test is guarded by a 60 s watchdog.

## Extra tests to write
1. Two nodes with 1 vnode each, key hash below, between and above both positions. Catches an off-by-one in the `range` bound.
2. `get_replicas(key, 2)` on a ring where one node's vnodes are adjacent, so the walk must skip same-node positions. Catches a walk that returns the same node twice.
3. Three nodes where `a` and `c` collide on a position and `b` does not. Remove `a`, then check `c` owns the position. Catches a ring that forgets the losing claimant.
4. Add the same node 100 times, then `vnode_count()` equals the count after one add. Catches double insertion.
5. Remove the last node, then `get` returns `None` and `vnode_count()` is 0. Catches leaked positions.
6. A proptest with a random sequence of add and remove operations, compared against a brute-force model that recomputes all positions from the node set. Catches state drift in incremental updates.
7. Keys that are empty or contain non-UTF-8 bytes. Catches an accidental `str` conversion.
8. Print the load spread for 8, 16 and 160 vnodes on 10 nodes and record the numbers in your notes. Shows how vnode count controls balance.

## Stretch (optional)
1. Weighted nodes: `add_weighted(name, weight)` gives `weight * vnodes_per_node` vnodes. Check that load is proportional to weight within 15%.
2. Bounded-load hashing (https://arxiv.org/abs/1608.01350): keep a per-node load count and cap it at `ceil(1.25 * average)`. When the owner is full, take the next distinct node clockwise. Check that no node exceeds the cap while inserting 100,000 keys.
3. Jump consistent hash (https://arxiv.org/abs/1406.2294): implement it and compare key movement on growing from 10 to 11 buckets, memory use, and what it cannot do (removing an arbitrary node).
