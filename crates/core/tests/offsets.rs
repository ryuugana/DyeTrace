//! Offset table rules.

use dyetrace_core::offsets;

#[test]
fn pointer_slot_is_always_present() {
    let table = offsets::from_boxes(&[], (100, 100));
    assert_eq!(table, vec![(0, 0)]);
}

#[test]
fn a_box_yields_its_crosshair_offset() {
    // A box is drawn 10 px to the right of the crosshair it belongs to, so a
    // box 60 px right of the pointer means a sample at +50.
    let table = offsets::from_boxes(&[(160, 80)], (100, 100));
    assert!(table.contains(&(50, -20)), "got {table:?}");
    assert!(table.contains(&(0, 0)), "got {table:?}");
}

#[test]
fn near_duplicates_collapse() {
    let table = offsets::from_boxes(&[(160, 80), (162, 81)], (100, 100));
    assert_eq!(table.len(), 2, "got {table:?}");
}

#[test]
fn table_never_exceeds_five_slots() {
    let boxes = [
        (160, 80),
        (60, 80),
        (160, 160),
        (60, 160),
        (110, 40),
        (110, 170),
    ];
    assert!(offsets::from_boxes(&boxes, (100, 100)).len() <= 5);
}
