//! Does the arena get memory back, and can it give it out again?
//!
//! The second question is the one that matters. A policy that evicts
//! perfectly into an arena that cannot reuse the space has not helped,
//! and "free bytes" is the number that hides it: after enough ragged
//! evictions an arena can be half empty and unable to place anything.
//! So the measurement is `largest`, the biggest single run, against the
//! total free.

use mlos_arena::Arena;

/// An arena of `bytes`, leaked because that is the lifetime it wants.
fn arena(bytes: usize) -> Arena {
    Arena::new(Box::leak(vec![0u8; bytes].into_boxed_slice()))
}

#[test]
fn a_fresh_arena_is_one_run() {
    let held = arena(4096);
    let free = held.occupancy();
    assert_eq!(free.used, 0);
    assert_eq!(free.capacity, 4096);
    assert_eq!(free.largest, 4096, "nothing placed, so nothing divided");
}

#[test]
fn what_is_released_comes_back() {
    let mut held = arena(4096);
    let (at, _) = held.place(1024).expect("room");
    assert_eq!(held.occupancy().used, 1024);
    held.release(at, 1024).expect("it was ours");
    assert_eq!(held.occupancy().used, 0);
    assert_eq!(held.occupancy().largest, 4096, "and merges back into one");
}

#[test]
fn adjacent_releases_coalesce() {
    // The property that keeps the free list short. Without it, a thousand
    // evictions of adjacent tiles would leave a thousand holes and the
    // fixed capacity would run out on a workload that never fragmented.
    let mut held = arena(4096);
    let placed: Vec<u64> = (0..4).map(|_| held.place(1024).expect("room").0).collect();
    for at in &placed {
        held.release(*at, 1024).expect("ours");
    }
    assert_eq!(held.occupancy().largest, 4096, "four holes became one");
}

#[test]
fn released_space_is_handed_out_again() {
    // Eviction is pointless if the bytes cannot be reused.
    let mut held = arena(2048);
    let (first, _) = held.place(1024).expect("room");
    held.place(1024).expect("room");
    assert!(held.place(1024).is_err(), "full");
    held.release(first, 1024).expect("ours");
    let (again, _) = held.place(1024).expect("the evicted space");
    assert_eq!(again, first, "and it is the same bytes");
}

#[test]
fn a_hole_too_small_is_not_used() {
    // First fit must fit. Handing out a run shorter than asked for would
    // be the kind of bug that corrupts the object placed next to it.
    let mut held = arena(4096);
    let (a, _) = held.place(512).expect("room");
    held.place(512).expect("room");
    held.release(a, 512).expect("ours");
    let (at, room) = held.place(1024).expect("room further on");
    assert_ne!(at, a, "the 512-byte hole cannot hold 1024");
    assert_eq!(room.len(), 1024);
}

#[test]
fn uniform_tiles_do_not_fragment() {
    // The easy case, and worth pinning because the workload MLOS runs
    // today is exactly this: 1 KiB tiles into a 32 KiB arena. Evict every
    // other one and the arena is half free in alternating holes; evict
    // the rest and it is one run again.
    let mut held = arena(32 * 1024);
    let placed: Vec<u64> = (0..32).map(|_| held.place(1024).expect("room").0).collect();
    assert_eq!(held.occupancy().largest, 0, "full");

    for at in placed.iter().step_by(2) {
        held.release(*at, 1024).expect("ours");
    }
    let free = held.occupancy();
    assert_eq!(free.capacity - free.used, 16 * 1024, "half free");
    assert_eq!(free.largest, 1024, "and every hole is one tile wide");

    for at in placed.iter().skip(1).step_by(2) {
        held.release(*at, 1024).expect("ours");
    }
    assert_eq!(held.occupancy().largest, 32 * 1024, "all one run again");
}

#[test]
fn ragged_sizes_do_fragment_and_it_is_measurable() {
    // The hard case, which KV blocks growing with context will be. Place
    // a mix, free every other one, and report what fraction of the free
    // space is reachable in one run -- the number a policy's success
    // should be discounted by.
    let mut held = arena(64 * 1024);
    let sizes: Vec<u32> = (0..32).map(|n| 256 * (1 + n % 7)).collect();
    let placed: Vec<(u64, u32)> = sizes
        .iter()
        .filter_map(|size| held.place(*size).ok().map(|(at, _)| (at, *size)))
        .collect();

    for (at, size) in placed.iter().step_by(2) {
        held.release(*at, *size).expect("ours");
    }
    let free = held.occupancy();
    let spare = free.capacity - free.used;
    let reachable = free.largest * 100 / spare;
    println!(
        "{} B free, largest run {} B -- {reachable}% reachable in one piece",
        spare, free.largest
    );
    assert!(spare > 0 && free.largest > 0);
    // Not an assertion about the number, which depends on the sizes. What
    // must hold is that the arena can still say how bad it is.
    assert!(free.largest <= spare);
}

#[test]
fn a_release_the_free_list_cannot_record_is_refused() {
    // And the object stays resident. Losing the bytes would leave the
    // accounting claiming memory nothing can hand out, which is worse
    // than refusing to evict.
    let mut held = arena(mlos_arena::HOLES * 2048);
    let placed: Vec<u64> = (0..mlos_arena::HOLES * 2)
        .filter_map(|_| held.place(512).ok().map(|(at, _)| at))
        .collect();
    let mut refused = 0;
    for at in placed.iter().step_by(2) {
        if held.release(*at, 512).is_err() {
            refused += 1;
        }
    }
    assert!(refused > 0, "the free list should have filled");
    // Whatever it accepted is still consistent.
    let free = held.occupancy();
    assert!(free.used <= free.capacity);
}
