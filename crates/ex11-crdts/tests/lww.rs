use ex11_crdts::{Crdt, LwwRegister};

#[test]
fn new_register_returns_its_value() {
    let r = LwwRegister::new("a", 3, 1);
    assert_eq!(*r.get(), "a");
}

#[test]
fn newer_timestamp_wins() {
    let mut r = LwwRegister::new("a", 3, 1);
    r.set("b", 4, 1);
    assert_eq!(*r.get(), "b");
}

#[test]
fn older_timestamp_is_ignored_even_from_a_higher_replica() {
    let mut r = LwwRegister::new("a", 3, 1);
    r.set("b", 2, 9);
    assert_eq!(*r.get(), "a");
}

/// (current ts, current replica, write ts, write replica, write applies)
const TIE_TABLE: [(u64, u32, u64, u32, bool); 8] = [
    (5, 1, 5, 2, true),  // same ts, higher replica wins
    (5, 2, 5, 1, false), // same ts, lower replica loses
    (5, 1, 5, 1, false), // identical pair, different value: ignored
    (5, 1, 6, 0, true),  // higher ts beats any replica id
    (5, 1, 4, 9, false), // lower ts loses to any replica id
    (0, 0, 0, 1, true),
    (0, 0, 0, 0, false),
    (u64::MAX, 3, u64::MAX, 4, true),
];

#[test]
fn set_follows_the_tie_break_table() {
    for (cts, crep, wts, wrep, applies) in TIE_TABLE {
        let mut r = LwwRegister::new("old", cts, crep);
        r.set("new", wts, wrep);
        let expect = if applies { "new" } else { "old" };
        assert_eq!(
            *r.get(),
            expect,
            "current=({cts},{crep}) write=({wts},{wrep})"
        );
    }
}

#[test]
fn merge_follows_the_tie_break_table() {
    for (cts, crep, wts, wrep, applies) in TIE_TABLE {
        let mut r = LwwRegister::new("old", cts, crep);
        let other = LwwRegister::new("new", wts, wrep);
        r.merge(&other);
        let expect = if applies { "new" } else { "old" };
        assert_eq!(
            *r.get(),
            expect,
            "current=({cts},{crep}) other=({wts},{wrep})"
        );
    }
}

#[test]
fn merge_after_a_losing_set_keeps_the_winner_in_both_orders() {
    let mut a = LwwRegister::new(0u8, 0, 1);
    let mut b = LwwRegister::new(0u8, 0, 2);
    a.set(10, 7, 1);
    b.set(20, 7, 2);
    let mut ab = a.clone();
    ab.merge(&b);
    let mut ba = b.clone();
    ba.merge(&a);
    assert_eq!(ab, ba);
    assert_eq!(*ab.get(), 20);
}

#[test]
fn merge_does_not_modify_the_argument() {
    let mut a = LwwRegister::new(1u8, 1, 1);
    let b = LwwRegister::new(2u8, 2, 2);
    let b_before = b.clone();
    a.merge(&b);
    assert_eq!(b, b_before);
    assert_eq!(a, b);
}
