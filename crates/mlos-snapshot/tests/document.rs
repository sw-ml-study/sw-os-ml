//! What the running emitter must produce, whatever the numbers are.
//!
//! The figures move every time the arena budget or the model shape does.
//! These are the properties that must not: the shared validator passes,
//! the vocabulary stays inside the closed set sw-mlpl colours from, the
//! arena's regions describe what is actually resident, and the edge table
//! joins a stored tile to the bytes it became.

mod fixture;

use mlos_layout::{Columns, validate};

/// A budget a quarter the size of the model's weights, as `mlos-lab` uses.
const BUDGET: usize = 32 * 1024;

/// The document from a manager with `acquired` tiles faulted in.
fn document(acquired: u32) -> String {
    fixture::document(&fixture::with(BUDGET, acquired))
}

#[test]
fn it_satisfies_the_same_contract_as_the_static_emitter() {
    // The point of the shared validator: two emitters with no code in
    // common, held to one definition of the format.
    validate(&document(40)).expect("a running layout satisfies the contract");
    validate(&document(0)).expect("so does one where nothing is resident");
}

#[test]
fn nothing_resident_still_tiles_the_arena() {
    let text = document(0);
    let found = Columns::read(&text);
    let dram: Vec<&str> = found
        .strings("region_space")
        .expect("a space column")
        .into_iter()
        .filter(|space| *space == "dram")
        .collect();
    // Exactly one region: the whole arena, free. An empty space with no
    // regions at all would tile nothing and fail the contract.
    assert_eq!(dram.len(), 1, "in:\n{text}");
    validate(&text).expect("an untouched arena is still a valid document");
}

#[test]
fn a_full_arena_has_no_free_region() {
    let text = document(40);
    let found = Columns::read(&text);
    let kinds = found.strings("region_kind").expect("a kind column");
    let lengths = found.numbers("region_length").expect("a length column");
    // A zero-length region satisfies the tiling rule and draws as nothing,
    // which is a box in the legend that is never on screen.
    assert!(!lengths.contains(&0), "a zero-length region in:\n{text}");
    assert!(kinds.contains(&"weight-tile"));
}

#[test]
fn the_arena_holds_what_the_table_says_is_resident() {
    let manager = fixture::with(BUDGET, 40);
    let text = fixture::document(&manager);
    let found = Columns::read(&text);
    let space = found.strings("region_space").expect("a space column");
    let state = found.strings("region_state").expect("a state column");

    let in_arena = space.iter().zip(&state).filter(|(s, _)| **s == "dram");
    let resident = in_arena.filter(|(_, st)| **st == "resident").count();
    let used = manager.arena.occupancy().used;
    // Each tile is exactly one arena granule wide, so the count and the
    // occupancy have to agree. They are read from different places: one
    // from the emitted document, one from the arena itself.
    assert_eq!(resident as u64 * 1024, used, "in:\n{text}");
}

#[test]
fn every_edge_runs_from_stored_bytes_to_the_bytes_they_became() {
    let text = document(40);
    let found = Columns::read(&text);
    let ids = found.numbers("region_id").expect("an id column");
    let space = found.strings("region_space").expect("a space column");
    let from = found.numbers("rel_from").expect("a rel_from column");
    let to = found.numbers("rel_to").expect("a rel_to column");
    let kinds = found.strings("rel_kind").expect("a rel_kind column");

    let place = |id: u64| {
        let index = ids
            .iter()
            .position(|found| *found == id)
            .expect("a known id");
        space[index]
    };
    assert!(!from.is_empty(), "no edges at all in:\n{text}");
    assert!(kinds.iter().all(|kind| *kind == "backs"));
    for (from, to) in from.iter().zip(&to) {
        assert_eq!(place(*from), "disk", "an edge from somewhere else");
        assert_eq!(place(*to), "dram", "an edge to somewhere else");
        // Same object, different space: the ids differ only in the space
        // tag, which is what lets a consumer join the two documents.
        assert_eq!(from & 0x0fff_ffff, to & 0x0fff_ffff);
    }
}

#[test]
fn the_vocabulary_stays_inside_the_palette() {
    let text = document(40);
    let found = Columns::read(&text);
    // sw-mlpl colours from a closed set. A kind outside it renders as
    // nothing, silently, in somebody else's repository -- so the set is
    // pinned here and widening it is a deliberate edit with a handoff.
    let kinds = ["weight-tile", "activation", "free"];
    for kind in found.strings("region_kind").expect("a kind column") {
        assert!(kinds.contains(&kind), "{kind:?} is not in the palette");
    }
    for state in found.strings("region_state").expect("a state column") {
        assert!(
            ["resident", "evicted", "never", "fixed"].contains(&state),
            "{state:?}"
        );
    }
    for tier in found.strings("region_tier").expect("a tier column") {
        assert!(
            ["hot", "warm", "cold", "stream", "archive", ""].contains(&tier),
            "{tier:?}"
        );
    }
}

#[test]
fn no_value_needs_json_escaping() {
    let text = document(40);
    // The `no_std` emitter does not escape, on the grounds that nothing it
    // writes needs it. That is only true while it stays true, so it is
    // checked rather than assumed.
    for (name, values) in &Columns::read(&text).0 {
        for value in values {
            let body = value.trim_matches('"');
            assert!(
                !body.contains(['"', '\\']) && !body.contains(|c: char| c.is_control()),
                "{name} has {value:?}, which needs escaping"
            );
        }
    }
}
