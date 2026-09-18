//! The running object store, as a `sw-ml-study.system-layout` document.
//!
//! The counterpart to `mlos-image-map`. That one describes what the build
//! produced; this one describes what is actually in memory, and the
//! difference between the two files is the whole argument of
//! `docs/PRD.md`. A picture of a disk image is something any operating
//! system could be drawn from. A picture of which parts of a model are
//! resident, what each cost to fetch, how often each has been wanted and
//! when each is next needed is not.
//!
//! Both documents use the same region ids, from `mlos-spaces`, so a
//! consumer joins them: the tile at `disk` id X and the bytes it became at
//! `dram` id Y are the same object, and the `backs` edge says so.
//!
//! Streams to a `Write` and allocates nothing -- there is no allocator in
//! the kernel. It also must not DISTURB what it measures: reading the
//! table is not acquiring from it, so emitting faults nothing in and moves
//! no counter. `tests/undisturbed.rs` holds that.

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

/// Writes the running layout, or reports that there is nothing to write.
///
/// `false` when no model has been registered. An empty document would be a
/// truthful picture of nothing, and the shell can say something better.
///
/// `revision` is stamped into `provenance` so a rendered snapshot is
/// traceable. The kernel cannot know its own commit, so the host passes it
/// in through `/chosen/bootargs` -- the same channel that asked for this
/// document in the first place.
pub fn write(out: &mut impl Write, revision: &str) -> bool {
    mlos_lab::with(|manager| of(out, manager, revision)).is_some()
}

/// Emits, taking the revision from boot arguments -- the shell's verb.
///
/// Here rather than in `mlsh` because the boot argument and the
/// `provenance` field it fills are one decision, and splitting them across
/// two crates means two places to look when a snapshot comes back stamped
/// `unknown`.
pub fn write_for(out: &mut impl Write, bootargs: &str) -> bool {
    // One token: a revision has no spaces, and `mlsh.run=` may follow.
    let revision = mlos_machine::rest(bootargs, "mlos.rev=");
    write(out, revision.split_whitespace().next().unwrap_or_default())
}

/// The layout of any manager, not only the live one.
///
/// Public so a test can build its own arena and object table rather than
/// reaching for the kernel's statics. That matters more than it looks: the
/// live manager is a `static` shared by every test in a binary, and a test
/// suite that mutates it cannot run its cases in parallel or in isolation.
///
/// `provenance` comes LAST, which looks odd and is deliberate: every
/// column is written with a trailing comma, and a streaming emitter with
/// no buffer cannot go back and remove one. A scalar at the end closes the
/// object without it. Key order is not significant to any JSON parser, and
/// the two lines that identify the format are still the first two.
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
///
/// It makes the high-water mark a boundary in the picture rather than a
/// number somebody has to be told.
fn tail(used: u64, capacity: u64) -> Row {
    Row {
        place: Where::Dram,
        // The first structural id of the space, which is what the static
        // emitter's `fill` gives its free region too -- so the arena's
        // spare space is the same region in both documents.
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
