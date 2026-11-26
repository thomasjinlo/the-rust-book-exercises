use ex11_crdts::{Crdt, GCounter};

#[test]
fn new_counter_is_zero() {
    assert_eq!(GCounter::new().value(), 0);
}

#[test]
fn increments_from_several_replicas_add_up() {
    let mut c = GCounter::new();
    c.increment(1, 5);
    c.increment(2, 7);
    c.increment(1, 3);
    assert_eq!(c.value(), 15);
}

#[test]
fn merge_takes_pointwise_max_not_sum() {
    let mut a = GCounter::new();
    a.increment(1, 5);
    let mut b = GCounter::new();
    b.increment(1, 3);
    b.increment(2, 2);
    a.merge(&b);
    assert_eq!(a.value(), 7); // slot 1 = max(5, 3), slot 2 = 2
}

#[test]
fn merging_a_stale_copy_changes_nothing() {
    let mut a = GCounter::new();
    a.increment(1, 4);
    let stale = a.clone();
    a.increment(1, 6);
    let before = a.clone();
    a.merge(&stale);
    assert_eq!(a, before);
    assert_eq!(a.value(), 10);
}

#[test]
fn increment_by_zero_leaves_state_unchanged() {
    let mut c = GCounter::new();
    c.increment(9, 0);
    assert_eq!(c, GCounter::new());
}

#[test]
fn value_saturates_at_u64_max() {
    let mut c = GCounter::new();
    c.increment(1, u64::MAX);
    c.increment(2, 5);
    assert_eq!(c.value(), u64::MAX);
}

#[test]
fn slot_saturates_at_u64_max() {
    let mut c = GCounter::new();
    c.increment(1, u64::MAX);
    c.increment(1, 1);
    assert_eq!(c.value(), u64::MAX);
}

#[test]
fn merge_does_not_modify_the_argument() {
    let mut a = GCounter::new();
    let mut b = GCounter::new();
    b.increment(3, 8);
    let b_before = b.clone();
    a.merge(&b);
    assert_eq!(b, b_before);
    assert_eq!(a, b);
}
