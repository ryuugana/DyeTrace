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

#[test]
fn slots_are_numbered_the_way_the_dye_window_numbers_them() {
    // Measured offsets from a live roll, in the order raster detection finds
    // them: the client's numbering is 1 = cursor, 2 = top left, 3 = top right,
    // 4 = bottom left, 5 = bottom right.
    let measured = [(29, -45), (-44, 23), (0, 0), (-47, -48), (26, 30)];
    let (table, slots) = offsets::ordered(&measured);

    assert_eq!(
        table,
        vec![(0, 0), (-47, -48), (29, -45), (-44, 23), (26, 30)]
    );
    assert_eq!(slots, vec![0, 1, 2, 3, 4]);
}

#[test]
fn a_missing_sample_does_not_renumber_the_others() {
    // The top-left sample was not drawn this time; the top-right one is still
    // slot 3 and the bottom-left one still slot 4.
    let measured = [(-40, 27), (0, 0), (31, -45)];
    let (table, slots) = offsets::ordered(&measured);

    assert_eq!(table, vec![(0, 0), (31, -45), (-40, 27)]);
    assert_eq!(slots, vec![0, 2, 3]);
}

#[test]
fn two_samples_in_one_corner_take_separate_slots() {
    let measured = [(0, 0), (-30, -20), (-60, -40)];
    let (_, slots) = offsets::ordered(&measured);

    assert_eq!(slots.len(), 3);
    assert_eq!(slots[0], 0);
    assert_eq!(slots[1], 1, "the nearer one keeps the corner");
    assert_ne!(slots[2], 1);
}
