//! Looking at the system must not change it.
//!
//! The reason this gets its own file rather than one more assertion: the
//! numbers a snapshot reports are the numbers M3's policy comparison will
//! be judged on, and an emitter that faulted an object in to describe it
//! would inflate exactly the counters it was measuring. The failure would
//! be invisible -- every document would still be valid, tile, and parse.
//!
//! The risk is real and not hypothetical. `Manager::acquire` is one letter
//! away from `Manager::table.get`, the fault path is what makes an object
//! resident, and `region_start` for an arena region is only meaningful
//! once something has been placed there. An emitter written the obvious
//! way -- "to describe this object, get it" -- would pass every other test
//! in this crate.

mod fixture;

use mlos_layout::Columns;

/// A budget a quarter the size of the model's weights.
const BUDGET: usize = 32 * 1024;

#[test]
fn emitting_faults_nothing_in_and_moves_no_counter() {
    let manager = fixture::with(BUDGET, 12);
    let before = manager.counters.report();
    let occupancy = manager.arena.occupancy();
    let fault = manager.last_fault.map(|fault| (fault.layer, fault.tensor));

    // Twice, because a lazy emitter that populated something on first use
    // would look clean on a single pass.
    let first = fixture::document(&manager);
    let second = fixture::document(&manager);

    let after = manager.counters.report();
    assert_eq!(
        after.total_faults(),
        before.total_faults(),
        "a fault was serviced"
    );
    assert_eq!(after.resident, before.resident, "residency moved");
    assert_eq!(after.registered, before.registered, "the table grew");
    assert_eq!(manager.arena.occupancy(), occupancy, "the arena grew");
    assert_eq!(
        manager.last_fault.map(|fault| (fault.layer, fault.tensor)),
        fault,
        "the last fault was overwritten, so something was acquired"
    );
    assert_eq!(first, second, "two snapshots of an unchanged system differ");
}

#[test]
fn a_snapshot_reports_what_it_found_rather_than_what_it_wanted() {
    // An emitter that acquired to describe would report the arena full and
    // every tile resident, which is the specific wrong answer worth
    // naming. With a 32 KiB budget only 32 of the 128 tiles can fit.
    let manager = fixture::with(BUDGET, 12);
    let text = fixture::document(&manager);
    let found = Columns::read(&text);
    let states = found.strings("region_state").expect("a state column");

    let resident = states.iter().filter(|state| **state == "resident").count();
    let never = states.iter().filter(|state| **state == "never").count();
    // Twelve tiles acquired, so twelve stored regions and twelve arena
    // regions say resident; the rest of the model has never been touched.
    assert_eq!(resident, 24, "in:\n{text}");
    assert!(never > 0, "everything looks resident, which cannot be true");
}
