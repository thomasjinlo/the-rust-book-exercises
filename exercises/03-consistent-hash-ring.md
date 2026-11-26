# Exercise 03: Consistent hash ring

**Time:** 2 h | **Crate:** `crates/ex03-consistent-hash-ring`

## Task
Build a ring that maps byte-string keys to named nodes. When a node joins or leaves, only the keys that must
move change owner. You choose the hash function.

## Example
The owners depend on your hash, so this table lists only results that hold for any implementation. Let `x` be
the result of `get(b"user:42")` on the three-node ring.

| Call | Returns |
|---|---|
| `r = Ring::new(160)`; `r.get(b"user:42")` | `None` |
| `r.add_node("a")`, `r.add_node("b")`, `r.add_node("c")` | |
| `r.get(b"user:42")` | `Some(x)`, with `x` one of `"a"`, `"b"`, `"c"` |
| `r.remove_node("b")` | |
| `r.get(b"user:42")` | `x` if `x` is `"a"` or `"c"`, otherwise `"a"` or `"c"` |

A ring with one node `"solo"` returns `Some("solo")` for every key, including `b""`.

## API
Create the ring with `Ring::new(points_per_node: u32)`, where `points_per_node >= 1`. It is how many places on
the ring each node gets. Node names are non-empty. Keys are any bytes.

### `add_node`
```rust
pub fn add_node(&mut self, name: &str)
```
1. Makes `name` a node. If it already is one, nothing changes.
2. Every key keeps its owner or moves to `name`.

### `remove_node`
```rust
pub fn remove_node(&mut self, name: &str)
```
1. Removes `name`. If it is not a node, nothing changes.
2. No key is owned by `name` afterwards. Every key it did not own keeps its owner.

### `get`
```rust
pub fn get(&self, key: &[u8]) -> Option<&str>
```
1. On an empty ring, returns `None`. With one node, that node owns every key.
2. Answers depend only on `points_per_node` and the set of nodes, not on the order of `add_node` and
   `remove_node` calls.

## Done when
Edit only `src/lib.rs`. From `crates/ex03-consistent-hash-ring`, `cargo test` reports 9 passed (the untouched starter
fails all 9). With 10 nodes, 160 points each and 100,000 keys, one test needs every node to own 6,000 to 14,500
keys, and adding an 11th node to move between 6% and 13% of the keys.

## Read first
- [Ch. 8.3: Storing Keys with Associated Values in Hash Maps](https://doc.rust-lang.org/book/ch08-03-hash-maps.html)
- [Ch. 10.3: Validating References with Lifetimes](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html)
- [Consistent hashing (Wikipedia)](https://en.wikipedia.org/wiki/Consistent_hashing)

## Stretch (optional)
- Add `get_replicas(&self, key: &[u8], n: usize) -> Vec<&str>`: up to `n` different nodes, the owner first.
- Add `add_weighted(&mut self, name: &str, weight: u32)`, which gives a node `weight` times as many points.
