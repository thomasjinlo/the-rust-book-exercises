# Exercise 03: Consistent hash ring

**Time:** 2.5 h | **Crate:** `crates/ex03-consistent-hash-ring`

## Read first
**Rust Book**
- [Ch. 5: Using Structs to Structure Related Data](https://doc.rust-lang.org/book/ch05-00-structs.html)
- [Ch. 6: Enums and Pattern Matching](https://doc.rust-lang.org/book/ch06-00-enums.html)
- [Ch. 8: Common Collections](https://doc.rust-lang.org/book/ch08-00-common-collections.html)
- [Ch. 9: Error Handling](https://doc.rust-lang.org/book/ch09-00-error-handling.html)
- [Ch. 10.3: Validating References with Lifetimes](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html)
- [Ch. 11: Writing Automated Tests](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Ch. 13.2: Processing a Series of Items with Iterators](https://doc.rust-lang.org/book/ch13-02-iterators.html)

**Standard library**
- [`Option`](https://doc.rust-lang.org/std/option/enum.Option.html)

**Background**
- [Consistent hashing (Wikipedia)](https://en.wikipedia.org/wiki/Consistent_hashing): the ring, clockwise ownership, why adding or removing a node moves few keys, and virtual nodes.
- [Dynamo (Apache Cassandra Documentation)](https://cassandra.apache.org/doc/latest/cassandra/architecture/dynamo.html): how a distributed store uses the ring, walks it to find replicas, and gives each machine several tokens to even out load.

## Task
Build a consistent hash ring that maps byte-string keys to named nodes. It also lists the replica nodes for a key.
When a node joins or leaves, only the keys that must move should change owner. You choose the hash function.

## API
Node names are non-empty strings. Keys are any bytes, including empty. No call may panic, for any input.
Answers depend only on `points_per_node` and the set of current members, not on the order of `add_node` and
`remove_node` calls. Two rings with the same `points_per_node` and the same members give the same answers.

### `ZeroPoints`
```rust
pub struct ZeroPoints;
```
- Derives `Debug, Clone, Copy, PartialEq, Eq`. Implements `Display` and `std::error::Error`.

### `Ring::new`
```rust
pub fn new(points_per_node: u32) -> Result<Ring, ZeroPoints>
```
| Input | Returns |
|---|---|
| `points_per_node == 0` | `Err(ZeroPoints)` |
| otherwise | `Ok` with an empty ring |

`Ring` derives `Debug, Clone`. `points_per_node` is how many places on the ring each node gets.

### `Ring::add_node`
```rust
pub fn add_node(&mut self, name: &str) -> bool
```
| Input | Returns | Effect |
|---|---|---|
| `name` is not a member | `true` | `name` becomes a member |
| `name` is a member | `false` | none |

### `Ring::remove_node`
```rust
pub fn remove_node(&mut self, name: &str) -> bool
```
| Input | Returns | Effect |
|---|---|---|
| `name` is a member | `true` | `name` is no longer a member |
| `name` is not a member | `false` | none |

### `Ring::node_count`
```rust
pub fn node_count(&self) -> usize
```
Returns the number of members.

### `Ring::get`
```rust
pub fn get(&self, key: &[u8]) -> Option<&str>
```
1. On an empty ring, returns `None`.
2. Otherwise returns the name of one current member. The same ring returns the same name for the same key every
   time. With one member, that member owns every key.
3. After `remove_node(x)` returns `true`: no key is owned by `x`, and every key that `x` did not own keeps its
   owner.
4. After `add_node(x)` returns `true` on a ring that had members: every key keeps its owner or moves to `x`.
   Removing `x` and adding it back restores every earlier answer.
5. With `points_per_node = 160`, members `node-0` to `node-9`, and keys `key-0` to `key-99999` (as bytes):
   - Adding `node-10` changes the owner of 6% to 13% of the keys (inclusive). A fair share is 9.1%.
   - Before adding it, every node owns between 6,000 and 14,500 keys, and none owns zero. An even split is 10,000.

### `Ring::get_replicas`
```rust
pub fn get_replicas(&self, key: &[u8], n: usize) -> Vec<&str>
```
1. Returns `min(n, node_count())` names, all different members. Empty if `n == 0` or the ring is empty.
2. If the result is not empty, its first name equals `get(key)`.
3. After `remove_node(x)`, `x` is in no result.

## Example
The owners depend on your hash, so this table uses only results that hold for any valid implementation. Let
`x` be the result of `get(b"user:42")` on the three-node ring.

| Call | Returns |
|---|---|
| `Ring::new(0)` | `Err(ZeroPoints)` |
| `r = Ring::new(160).unwrap()`; `r.get(b"user:42")` | `None` |
| `r.get_replicas(b"user:42", 3)` | `[]` |
| `r.add_node("a")`, `r.add_node("b")`, `r.add_node("c")` | `true`, `true`, `true` |
| `r.add_node("a")` | `false` |
| `r.node_count()` | `3` |
| `r.get(b"user:42")` | `Some(x)`, with `x` one of `"a"`, `"b"`, `"c"` |
| `r.get_replicas(b"user:42", 0)` | `[]` |
| `r.get_replicas(b"user:42", 2)` | two different names, the first is `x` |
| `r.get_replicas(b"user:42", 9)` | `"a"`, `"b"`, `"c"` in some order, the first is `x` |
| `r.remove_node("b")` | `true` |
| `r.remove_node("b")` | `false` |
| `r.get(b"user:42")` | `x` if `x` is `"a"` or `"c"`; otherwise `"a"` or `"c"` |
| `r.get_replicas(b"user:42", 9)` | `"a"` and `"c"` in some order |

A ring with one member `"solo"` returns `Some("solo")` for `get(b"")` and `get(&[0xff, 0x00])`.

## Done when
Edit only `src/lib.rs`. Keep the public signatures. Do not edit `tests/`. The starter compiles, and all 14 of its
tests fail until you finish.

From `crates/ex03-consistent-hash-ring`, all of these exit 0:
```
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```
`cargo test` reports 14 passed.

## Extra tests (optional)
- 1,000 keys, `points_per_node = 1`, 5 nodes: `get_replicas(key, 5)` lists all five nodes once each.
- Add the same node 100 times: every answer equals the answer after one add.
- Remove the last member: `get` returns `None`.
- Add `a`, `b`, remove `b`, add `b`: every answer equals the answer before the removal.
- Keys that are empty or not valid UTF-8 get an owner.

## Stretch (optional)
1. Weighted nodes: `add_weighted(name, weight)` gives a node `weight` times as many points, and the share of keys is proportional to weight within 15%.
2. Bounded load: cap each node at `ceil(1.25 * average)` keys. When the owner is full, hand the key to the next distinct node ([Consistent Hashing with Bounded Loads](https://arxiv.org/abs/1608.01350)).
3. Make `Ring` safe to share across threads for lookups while a writer adds nodes, and test that readers never see a half-added node.
