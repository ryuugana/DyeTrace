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
fn without_a_choice_the_spot_count_decides() {
    let want = Rgb::new(1, 2, 3);
    let need = search::plan(5, want, 3, &[]);

    assert_eq!(need.count, 3);
    assert_eq!(need.asked(), 5);
    assert!(need.must.is_empty());
    assert!(need.colours.iter().all(|colour| *colour == Some(want)));
}

#[test]
fn fewer_slots_than_spots_guarantee_the_chosen_ones() {
    // Slot 2 chosen, three spots: slot 2 must match, and two more may come from
    // anywhere.
    let want = Rgb::new(1, 2, 3);
    let need = search::plan(5, want, 3, &[1]);

    assert_eq!(need.count, 3);
    assert_eq!(need.asked(), 5);
    assert_eq!(need.must, vec![1]);
}

#[test]
fn as_many_slots_as_spots_guarantee_each_one() {
    let want = Rgb::new(1, 2, 3);
    let need = search::plan(5, want, 3, &[0, 2, 4]);

    assert_eq!(need.count, 3);
    assert_eq!(need.must, vec![0, 2, 4]);
    assert_eq!(need.asked(), 5);
}

#[test]
fn more_slots_than_spots_only_count_the_chosen_ones() {
    // Slots 1, 3 and 5 chosen with two spots: any two of the three will do.
    let want = Rgb::new(1, 2, 3);
    let need = search::plan(5, want, 2, &[0, 2, 4]);

    assert_eq!(need.count, 2);
    assert!(need.must.is_empty());
    assert_eq!(need.asked(), 3);
    assert_eq!(need.colours[0], Some(want));
    assert_eq!(need.colours[1], None);
    assert_eq!(need.colours[2], Some(want));
    assert_eq!(need.colours[3], None);
    assert_eq!(need.colours[4], Some(want));
}

#[test]
fn a_slot_the_table_does_not_have_falls_back_to_the_spot_count() {
    let need = search::plan(4, Rgb::new(1, 2, 3), 2, &[4]);

    assert_eq!(need.count, 2);
    assert_eq!(need.asked(), 4);
    assert!(need.must.is_empty());
}

#[test]
fn the_requirement_cannot_exceed_the_table() {
    let need = search::plan(3, Rgb::black(), 5, &[]);
    assert_eq!(need.count, 3);
}

#[test]
fn a_guaranteed_slot_that_does_not_match_rejects_the_position() {
    // Colour at (10,10) only. Slot 1 is the pointer itself, so the only
    // candidate is (10,10); asking for slot 2 as well moves the sample 5 px up
    // and finds nothing.
    let mut grid = Image::filled(256, 256, Rgb::black());
    let want = Rgb::new(210, 30, 30);
    grid.put(10, 10, want);

    let offsets = [(0, 0), (0, -5)];
    let one = search::plan(2, want, 1, &[0]);
    let found = search::search_need(&grid, &offsets, &one, 0).expect("one slot");
    assert!(found.met);
    assert_eq!((found.at.x, found.at.y), (10, 10));

    grid.put(10, 5, want);
    let both = search::plan(2, want, 2, &[0, 1]);
    let found = search::search_need(&grid, &offsets, &both, 0).expect("both slots");
    assert!(found.met);
    assert_eq!((found.at.x, found.at.y), (10, 10));

    grid.put(10, 5, Rgb::black());
    let found = search::search_need(&grid, &offsets, &both, 0).expect("a best effort");
    assert!(!found.met, "slot 2 no longer holds the colour");
    assert_eq!(found.at.count, 1);
}

#[test]
fn more_chosen_slots_than_spots_accepts_any_subset() {
    let mut grid = Image::filled(256, 256, Rgb::black());
    let want = Rgb::new(210, 30, 30);

    // Three chosen samples; only two of them hold the colour at (40, 40).
    grid.put(40, 40, want);
    grid.put(40, 36, want);   // sample 2 is 4 px up
    let offsets = [(0, 0), (0, -4), (0, -30)];
    let need = search::plan(3, want, 2, &[0, 1, 2]);

    let found = search::search_need(&grid, &offsets, &need, 0).expect("two of three is enough");
    assert!(found.met);
    assert_eq!((found.at.x, found.at.y), (40, 40));
    assert_eq!(found.at.count, 2);
}
