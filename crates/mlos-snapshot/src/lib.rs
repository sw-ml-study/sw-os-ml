//! The running object store, as a `sw-ml-study.system-layout` document.
//!
//! Invariant: streams to a `Write`, allocates nothing, and does not
//! disturb what it measures: no fault, no counter moved. Design and
//! history: docs/notes/mlos-snapshot.md.

#![no_std]
#![forbid(unsafe_code)]

mod columns;
mod parts;
mod rows;

use core::fmt::Write;

use mlos_objman::Manager;
use mlos_objtab::NextUse;
use mlos_spaces::{PRODUCER, SCHEMA, VERSION, Where};

use rows::Row;

/// Writes the running layout, stamped with `revision`. `false` when no
/// model has been registered and nothing was written.
pub fn write(out: &mut impl Write, revision: &str) -> bool {
    mlos_lab::with(|manager| of(out, manager, revision)).is_some()
}

/// The shell's verb: emits, taking the revision from `mlos.rev=` in the
/// boot arguments.
pub fn write_for(out: &mut impl Write, bootargs: &str) -> bool {
    // One token: `mlsh.run=` may follow the revision.
    let revision = mlos_machine::rest(bootargs, "mlos.rev=");
    write(out, revision.split_whitespace().next().unwrap_or_default())
}

/// The layout of any manager, not only the live one. `provenance` must
/// stay last: it is the one scalar that closes the object after the
/// columns' trailing commas.
pub fn of<const N: usize>(out: &mut impl Write, manager: &Manager<'static, N>, revision: &str) {
    let arena = manager.arena.occupancy();
    let (used, capacity) = (arena.used, arena.capacity);
    let stored = u64::from(mlos_lab::LAYERS) * u64::from(mlos_lab::TILES) * TILE;
    let (base, tail) = (arena.base, tail(used, capacity));

    let _ = write!(
        out,
        "{{\r\n  \"schema\": \"{SCHEMA}\",\r\n  \"version\": {VERSION},\r\n\r\n"
    );
    parts::spaces(out, stored, capacity);
    let _ = out.write_str("\r\n");
    parts::contract(out, manager, base, tail);
    parts::extras(out, manager, base, tail);
    let _ = out.write_str("\r\n");
    parts::edges(out, manager, base, tail);
    let _ = write!(
        out,
        "\r\n  \"provenance\": {{ \"producer\": \"{PRODUCER}\", \"revision\": \"{revision}\" }}\r\n}}\r\n"
    );
}

/// The arena's unused tail: the one region that describes no object.
fn tail(used: u64, capacity: u64) -> Row {
    Row {
        place: Where::Dram,
        // The first structural id of the space: the same id the static
        // emitter's `fill` gives its free region.
        id: Where::Dram.ids().next().unwrap_or_default(),
        start: used,
        length: capacity.saturating_sub(used),
        kind: "free",
        owner: "",
        tier: "",
        state: "fixed",
        object: 0,
        at: (0, 0),
        next_use: NextUse::Never,
        reuse: 0,
        cost: 0,
    }
}

/// Bytes per stored tile, as a `u64`.
const TILE: u64 = mlos_lab::TILE_BYTES as u64;
