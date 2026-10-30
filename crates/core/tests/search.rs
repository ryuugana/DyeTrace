//! Sampling and search rules.

use dyetrace_core::image::{Image, Rgb};
use dyetrace_core::search;

#[test]
fn wrap_is_modulo_256() {
    assert_eq!(search::wrap256(-1), 255);
    assert_eq!(search::wrap256(-39), 217);
    assert_eq!(search::wrap256(257), 1);
}

#[test]
fn separator_and_background_are_unreadable() {
    let grid = Image::filled(256, 256, Rgb::new(1, 2, 3));
    assert!(search::sample(&grid, 254, 10).is_none());
    assert!(search::sample(&grid, 10, 255).is_none());
    // -2 wraps to the separator column
    assert!(search::sample(&grid, -2, 10).is_none());
    assert_eq!(search::sample(&grid, 10, 10), Some(Rgb::new(1, 2, 3)));
}

#[test]
fn at_least_n_is_monotone() {
    // One readable sample holds the colour; an unreadable one must not stop the
    // count from reaching the requirement.
    let mut grid = Image::filled(256, 256, Rgb::black());
    grid.put(10, 10, Rgb::new(9, 9, 9));
    let offsets = [(0, 0), (-2, 0)];
    let colours = [Some(Rgb::new(9, 9, 9)), Some(Rgb::new(9, 9, 9))];
    let (count, _) = search::evaluate(&grid, &offsets, &colours, 0, 10, 10);
    assert_eq!(count, 1);
}

#[test]
fn best_position_wins_on_count() {
    let mut grid = Image::filled(256, 256, Rgb::new(0, 0, 0));
    let want = Rgb::new(200, 40, 40);
    // two adjacent positions hold the colour, one holds it twice
    grid.put(50, 50, want);
    grid.put(51, 50, want);
    grid.put(53, 50, want);

    let offsets = [(0, 0), (1, 0)];
    let found = search::search(&grid, &offsets, want, 0).unwrap();
    assert_eq!((found.x, found.y), (50, 50));
    assert_eq!(found.count, 2);
}

#[test]
fn a_chosen_slot_overrides_the_slot_count() {
    let want = Rgb::new(1, 2, 3);
    let (colours, need) = search::plan(5, want, 4, &[2]);

    assert_eq!(need, 1, "one sample is enough when one is chosen");
    assert_eq!(colours.len(), 5);
    assert_eq!(colours[2], Some(want));
    assert_eq!(colours.iter().filter(|c| c.is_some()).count(), 1);
}

#[test]
fn several_chosen_slots_must_all_hold_the_colour() {
    let want = Rgb::new(1, 2, 3);
    let (colours, need) = search::plan(5, want, 4, &[0, 2, 4]);

    assert_eq!(need, 3);
    assert_eq!(colours[0], Some(want));
    assert_eq!(colours[2], Some(want));
    assert_eq!(colours[4], Some(want));
    assert_eq!(colours[1], None);
    assert_eq!(colours[3], None);
}

#[test]
fn without_a_choice_the_count_decides() {
    let want = Rgb::new(1, 2, 3);
    let (colours, need) = search::plan(5, want, 3, &[]);

    assert_eq!(need, 3);
    assert!(colours.iter().all(|c| *c == Some(want)));
}

#[test]
fn a_slot_the_table_does_not_have_falls_back_to_the_count() {
    let (colours, need) = search::plan(4, Rgb::new(1, 2, 3), 2, &[4]);

    assert_eq!(need, 2);
    assert_eq!(colours.len(), 4);
    assert!(colours.iter().all(|c| c.is_some()));
}

#[test]
fn only_the_reachable_chosen_slots_are_asked_for() {
    let want = Rgb::new(1, 2, 3);
    let (colours, need) = search::plan(3, want, 5, &[1, 4]);

    assert_eq!(need, 1);
    assert_eq!(colours[1], Some(want));
    assert_eq!(colours.iter().filter(|c| c.is_some()).count(), 1);
}

#[test]
fn the_requirement_cannot_exceed_the_table() {
    let (_, need) = search::plan(3, Rgb::black(), 5, &[]);
    assert_eq!(need, 3);
}
