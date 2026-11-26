use ex11_crdts::{Crdt, OrSet};

#[test]
fn add_then_contains() {
    let mut s = OrSet::new();
    assert!(!s.contains(&"x"));
    s.add("x", 1);
    assert!(s.contains(&"x"));
}

#[test]
fn elements_are_sorted_and_deduplicated() {
    let mut s = OrSet::new();
    for e in ["pear", "apple", "fig", "apple"] {
        s.add(e.to_string(), 1);
    }
    assert_eq!(s.elements(), vec!["apple", "fig", "pear"]);
}

#[test]
fn remove_deletes_every_local_tag() {
    let mut s = OrSet::new();
    s.add(1u32, 1);
    s.add(1u32, 1);
    s.remove(&1);
    assert!(!s.contains(&1));
    assert!(s.elements().is_empty());
}

#[test]
fn re_add_after_remove_is_present() {
    let mut s = OrSet::new();
    s.add(1u32, 1);
    s.remove(&1);
    s.add(1u32, 1);
    assert!(s.contains(&1));
}

#[test]
fn remove_of_an_unobserved_element_is_a_no_op() {
    let mut s = OrSet::new();
    s.add(1u32, 1);
    let before = s.clone();
    s.remove(&99);
    assert_eq!(s, before);
    let mut empty = OrSet::<u32>::new();
    empty.remove(&5);
    assert_eq!(empty, OrSet::new());
}

#[test]
fn removal_propagates_through_merge() {
    let mut a = OrSet::new();
    a.add("x", 1);
    let mut b = OrSet::new();
    b.merge(&a);
    b.remove(&"x");
    a.merge(&b);
    assert!(!a.contains(&"x"));
    assert_eq!(a, b);
}

#[test]
fn add_wins_over_a_concurrent_remove() {
    let mut a = OrSet::new();
    a.add("x", 1);
    let mut b = a.clone();
    a.remove(&"x"); // A removes the tag it observed
    b.add("x", 2); // B concurrently adds a new tag
    let mut ab = a.clone();
    ab.merge(&b);
    let mut ba = b.clone();
    ba.merge(&a);
    assert!(ab.contains(&"x"));
    assert!(ba.contains(&"x"));
    assert_eq!(ab, ba);
}

#[test]
fn a_stale_replica_cannot_resurrect_a_removed_tag() {
    let mut a = OrSet::new();
    a.add("x", 1);
    let stale = a.clone();
    a.remove(&"x");
    a.merge(&stale);
    assert!(!a.contains(&"x"));
}

#[test]
fn re_add_survives_merge_with_a_replica_holding_the_old_tag() {
    let mut a = OrSet::new();
    a.add("x", 1);
    let stale = a.clone();
    a.remove(&"x");
    a.add("x", 1); // fresh tag, not in `stale`
    a.merge(&stale);
    assert!(a.contains(&"x"));
}

#[test]
fn tags_are_unique_per_replica_across_merges() {
    let mut a = OrSet::new();
    a.add("x", 1);
    let mut b = OrSet::new();
    b.merge(&a); // b learns replica 1's counter
    b.add("y", 1);
    let mut c = a.clone();
    c.merge(&b);
    c.remove(&"x");
    assert!(c.contains(&"y"));
    assert!(!c.contains(&"x"));
}

#[test]
fn two_replicas_adding_the_same_element_keep_it_after_one_removes() {
    let mut a = OrSet::new();
    let mut b = OrSet::new();
    a.add(7u32, 1);
    b.add(7u32, 2);
    a.merge(&b);
    b.merge(&a);
    a.remove(&7);
    b.merge(&a);
    assert!(!b.contains(&7)); // a observed both tags before removing
    let mut c = OrSet::new();
    c.add(7u32, 3);
    b.merge(&c);
    assert!(b.contains(&7));
}

#[test]
fn merge_does_not_modify_the_argument() {
    let mut a = OrSet::new();
    let mut b = OrSet::new();
    b.add(1u32, 2);
    b.add(2u32, 2);
    b.remove(&1);
    let b_before = b.clone();
    a.merge(&b);
    assert_eq!(b, b_before);
    assert_eq!(a, b);
}
