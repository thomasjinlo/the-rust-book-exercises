# Exercise 11: State-based CRDTs

**Time:** ~3.5 h | **Area:** distributed systems | **Crate:** `crates/ex11-crdts`
**Builds on:** none (standalone)

## Read first
- Rust Book ch10, generics and traits: https://doc.rust-lang.org/book/ch10-00-generics.html and https://doc.rust-lang.org/book/ch10-02-traits.html
- Rust Book ch08, collections: https://doc.rust-lang.org/book/ch08-00-common-collections.html
- Rust Book ch11, testing: https://doc.rust-lang.org/book/ch11-00-testing.html
- `BTreeMap`: https://doc.rust-lang.org/std/collections/struct.BTreeMap.html and `BTreeSet`: https://doc.rust-lang.org/std/collections/struct.BTreeSet.html
- Concept overview: https://en.wikipedia.org/wiki/Conflict-free_replicated_data_type
- Shapiro et al., "A comprehensive study of Convergent and Commutative Replicated Data Types": https://inria.hal.science/inria-00555588 (read the state-based counter and OR-Set sections)
- `proptest`: https://docs.rs/proptest/latest/proptest/

## What you are building
A conflict-free replicated data type (CRDT) is a value that every replica updates locally, without locks or consensus. Replicas later exchange full states and `merge` them. If `merge` is commutative, associative and idempotent, replicas that have seen the same updates hold equal states, whatever the delivery order, duplication or delay. Collaborative editors, distributed caches and multi-primary databases use this to stay available during partitions.

You implement four state-based CRDTs: a grow-only counter, an increment/decrement counter, a last-writer-wins register and an observed-remove set with add-wins semantics. Then you check the three merge laws with property tests and a seeded five-replica simulation.

## Starting point
Edit these files only:
- `src/gcounter.rs`, `src/pncounter.rs`, `src/lww.rs`, `src/orset.rs`

Do not edit `src/lib.rs` (the `Crdt` trait, `ReplicaId` and the provided `all_pairs_sync`) or anything under `tests/`. Add your own tests in new files under `tests/`.

You may change private fields and add private helpers. You may not change public signatures. `OrSet` starts with a placeholder field: design its state yourself.

Run:
```
cd crates/ex11-crdts
cargo test --no-fail-fast
```
`--no-fail-fast` makes cargo run every test file even when an earlier one fails. On the unmodified starter all 49 given tests fail (each stops at `todo!()`) and the run ends in under a second.

## Requirements
All types derive `Debug`, `Clone` and `PartialEq` over their whole state. `merge(&mut self, other: &Self)` never modifies `other`. `ReplicaId` is `u32`. A replica id is used by one live replica at a time.

**GCounter**
- R1. `new()` has `value() == 0`.
- R2. State is one `u64` slot per replica id in a `BTreeMap<ReplicaId, u64>`. `increment(id, by)` adds `by` to slot `id`, saturating at `u64::MAX`.
- R3. `increment(id, 0)` leaves the state `==` to its previous state. It creates no slot.
- R4. `value()` is the sum of all slots, saturating at `u64::MAX`.
- R5. `merge` takes the pointwise maximum of the slots. A slot present on only one side is copied.

**PnCounter**
- R6. State is two `GCounter`s: increments and decrements. `increment(id, by)` and `decrement(id, by)` each add to one of them.
- R7. `value() -> i128` is `increments.value() - decrements.value()`, computed in `i128`. It cannot wrap. Each total saturates at `u64::MAX` first.
- R8. `merge` merges both `GCounter`s.

**LwwRegister<T: Clone + PartialEq>**
- R9. State is `(timestamp: u64, replica: ReplicaId, value: T)`. `new(value, timestamp, replica)` stores them. `get()` returns `&T`.
- R10. `set(value, timestamp, replica)` applies only if `(timestamp, replica)` is strictly greater than the stored pair, compared as a tuple: timestamp first, then replica id.
- R11. Consequences of R10: on equal timestamps the higher replica id wins. A higher timestamp beats any replica id. **A write whose pair equals the stored pair is ignored, even when its value differs.**
- R12. `merge` keeps the greater pair together with its value. On an equal pair it keeps `self`. This is only a join if equal pairs always carry equal values, which holds when each replica id writes a given timestamp at most once (R10 enforces that locally).

**OrSet<T: Ord + Clone>** (observed-remove set, add-wins)
- R13. `add(elem, id)` creates a tag `(id, n)`. `n` comes from a per-replica counter stored in the set. The counter starts at 1 for each id, increases by 1 per `add` with that id and is raised by `merge` to the maximum of both sides. A tag is never issued twice by sets that have merged each other's states.
- R14. Adding an element that is present adds a second tag. `elements()` still lists it once.
- R15. `remove(&elem)` removes exactly the tags of `elem` that this set holds now and records them as removed. Tags created elsewhere and not yet merged are unaffected.
- R16. `remove` of an element with no tags leaves the state `==` to its previous state.
- R17. `contains(&elem)` is true iff `elem` has at least one tag that is not removed.
- R18. `elements()` returns present elements in ascending `Ord` order, each once.
- R19. After `merge`, the live tags of every element are `(live_a ∪ live_b) \ (removed_a ∪ removed_b)`. A removed tag never comes back, whatever order or how often states are merged.
- R20. Add-wins: if replica A removes `x` while replica B concurrently adds `x`, `x` is present after A and B merge in either order.
- R21. After merging, two sets that have seen the same adds and removes are `==`. Hence the counter map and the removed-tag set take part in `==` and are merged (union, and per-id maximum).
- R22. Removed tags are kept forever. This is a known cost; see Stretch.

**Merge laws, for all four types**
- R23. Commutative: `a.merge(b)` and `b.merge(a)` give `==` states.
- R24. Associative: `(a ∪ b) ∪ c == a ∪ (b ∪ c)`.
- R25. Idempotent: `a ∪ a == a`.
- R26. Monotonic: `GCounter::merge` never lowers `value()`.

## Given tests
49 tests in 7 files, all through the public API.

| File | Tests | Checks |
|---|---|---|
| `tests/gcounter.rs` | 8: zero start, sum over replicas, max-not-sum merge, stale merge, zero increment, value saturation, slot saturation, argument untouched | R1-R5 |
| `tests/pncounter.rs` | 6: zero start, negative value, concurrent inc/dec, idempotent decrements, no wrap past `u64`, zero amounts | R6-R8 |
| `tests/lww.rs` | 7: initial get, newer wins, older ignored, 8-row tie-break table via `set`, same table via `merge`, merge order independence, argument untouched | R9-R12 |
| `tests/orset.rs` | 12: add/contains, sorted dedup, remove, re-add, unobserved remove, removal propagation, add-wins, stale replica cannot resurrect, re-add vs old tag, tag uniqueness across merges, two adders, argument untouched | R13-R21 |
| `tests/laws.rs` | 14 proptests: commutative, associative, idempotent for each type (12), GCounter monotonicity, OrSet merge-order independence. 256 cases each, fixed seed, random histories on 3 replicas with random state exchanges | R23-R26 |
| `tests/simulation.rs` | 2: 50 seeds of 5 replicas and 300 ops each, with stale, duplicated and reordered merges, then `all_pairs_sync`; and a small `all_pairs_sync` check | all |

The simulation also asserts after sync: `GCounter` equals the sum of increments, `PnCounter` equals increments minus decrements, `LwwRegister` holds the value of the last `set` (timestamps are unique), an element never added is absent, and an element added and never removed is present.

## Acceptance
```
cd crates/ex11-crdts
cargo test --no-fail-fast        # exit code 0, "49 passed" in total across the 7 result lines
cargo test --release             # exit code 0
cargo clippy --all-targets -- -D warnings   # exit code 0
cargo fmt --check                # exit code 0
```
Run `cargo test` 20 times in a row: it must pass every time. The tests are deterministic, so one failure is a bug. On the reference solution the whole suite takes about 1 s in debug mode.

## Extra tests to write
1. `GCounter`: replica 1 increments, replica 2 merges and increments, replica 1 merges back. Expect the total, not a double count. Catches merge by addition.
2. `OrSet`: A adds `x`; B merges A, removes `x`; C (never saw the remove) merges A and re-adds `x` under its own id; all three merge in every order. Expect `x` present everywhere. Catches removing by element instead of by tag.
3. `OrSet`: add `x` with id 1, remove it, add `x` with id 1 again; compare the two tags through merge with a copy taken before the remove. Catches a counter that is reset or derived from the live set size.
4. `OrSet`: merge A and B, then B and A, after A removed and B did not know. Compare with `==`. Catches a state that forgets removed tags and lets a stale replica resurrect them.
5. `LwwRegister<String>`: two replicas write at the same timestamp with different ids; merge in both orders and compare. Catches a tie-break that depends on argument order.
6. `PnCounter`: `increment(1, u64::MAX)` on three replica ids, expect `value()` of `u64::MAX` (saturated total), not `3 * u64::MAX`. Pin the choice in R4/R7.
7. Property test: build a replica from a random op list, then merge a clone of it into itself 3 times. Expect `==` to the original. Catches a merge that bumps a counter or clock.
8. A 3-replica partition test: replicas {A} and {B, C} work separately for 100 ops each, then heal with `all_pairs_sync`. Expect all equal.

## Stretch (optional)
1. Delta-state CRDTs: add `fn delta(&self, since: &VersionVector) -> Self` to `GCounter` and `OrSet` so replicas send only what the receiver lacks. Prove with a property test that merging the delta equals merging the full state.
2. Tombstone compaction: replace the removed-tag set with a causal context (a version vector plus a small set of out-of-order tags, a "dotted version vector"). A tag is dead if the context covers it and it is not live. Property test: compaction never changes `elements()` after any merge order.
3. Sketch an RGA sequence CRDT (collaborative text): each insert carries `(id, counter)` and the id of its left neighbour; concurrent inserts at the same position sort by tag, descending. Test that three replicas inserting at the head concurrently converge to one string.
