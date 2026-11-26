use ex11_crdts::{Crdt, PnCounter};

#[test]
fn new_counter_is_zero() {
    assert_eq!(PnCounter::new().value(), 0);
}

#[test]
fn decrement_can_go_negative() {
    let mut c = PnCounter::new();
    c.increment(1, 2);
    c.decrement(1, 5);
    assert_eq!(c.value(), -3);
}

#[test]
fn concurrent_inc_and_dec_merge_to_the_net() {
    let mut a = PnCounter::new();
    let mut b = PnCounter::new();
    a.increment(1, 10);
    b.decrement(2, 4);
    a.merge(&b);
    b.merge(&a);
    assert_eq!(a, b);
    assert_eq!(a.value(), 6);
}

#[test]
fn merge_is_idempotent_for_decrements() {
    let mut a = PnCounter::new();
    a.decrement(1, 3);
    let copy = a.clone();
    a.merge(&copy);
    a.merge(&copy);
    assert_eq!(a.value(), -3);
}

#[test]
fn value_does_not_wrap_past_u64() {
    let mut c = PnCounter::new();
    c.increment(1, u64::MAX);
    c.increment(2, 1);
    assert_eq!(c.value(), i128::from(u64::MAX));
    let mut d = PnCounter::new();
    d.decrement(1, u64::MAX);
    d.decrement(2, 1);
    assert_eq!(d.value(), -i128::from(u64::MAX));
}

#[test]
fn zero_amounts_leave_state_unchanged() {
    let mut c = PnCounter::new();
    c.increment(1, 0);
    c.decrement(1, 0);
    assert_eq!(c, PnCounter::new());
}
