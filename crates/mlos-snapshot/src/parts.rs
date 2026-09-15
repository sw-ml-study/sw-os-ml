//! The document's columns, in the order the contract lists them.
//!
//! Split the way the contract is: the columns every producer emits, then
//! the ones MLOS adds. A reader comparing this against
//! `../sw-mlpl/docs/storage-layout-viz.md`, or against the host emitter's
//! `render.rs`, should be able to do it by eye -- and the two emitters
//! disagreeing about a column name is the failure mode that would be
//! hardest to notice, since each is valid JSON on its own.

use core::fmt::Write;

use mlos_abi::ObjectClass;
use mlos_objman::Manager;
use mlos_spaces::{NextUseText, Where};
use mlos_virtio_blk::SECTOR;

use crate::{
    columns::{nums, strs, text},
    rows::{GRAIN, Row, rows},
};

/// The two spaces this document describes.
///
/// `sysram` is absent on purpose. A running kernel has no symbol table and
/// cannot say where its own `.text` ended, so claiming a RAM map here
/// would mean inventing one -- and the static document already has a real
/// one, drawn from the linked image.
pub fn spaces(out: &mut impl Write, stored: u64, capacity: u64) {
    let keys = [Where::Disk.key(), Where::Dram.key()];
    let names = ["virtio-blk model image", "object arena"];
    strs(out, "spaces", keys.into_iter(), |key| *key);
    strs(out, "space_name", names.into_iter(), |name| *name);
    nums(
        out,
        "space_block",
        [SECTOR as u64, GRAIN].into_iter(),
        |b| *b,
    );
    nums(out, "space_capacity", [stored, capacity].into_iter(), |c| {
        *c
    });
}

/// Image word counts, which MLOS has none of.
///
/// Emitted as zeros rather than omitted: a consumer written against SWTOS
/// reads them without checking, and a missing column is a crash where a
/// zero is a fact.
const WORDS: [&str; 3] = ["region_text_words", "region_data_words", "region_bss_words"];

/// The columns the contract itself defines.
pub fn contract<const N: usize>(
    out: &mut impl Write,
    manager: &Manager<'static, N>,
    base: u64,
    tail: Row,
) {
    let every = || rows(manager, base, tail);
    nums(out, "region_id", every(), |row| u64::from(row.id));
    nums(out, "region_start", every(), |row| row.start);
    nums(out, "region_length", every(), |row| row.length);
    for name in WORDS {
        nums(out, name, every(), |_| 0);
    }
    strs(out, "region_space", every(), |row| row.place.key());
    strs(out, "region_kind", every(), |row| row.kind);
    strs(out, "region_owner", every(), |row| row.owner);
    text(out, "region_name", every(), |out, row| {
        let _ = match row.object {
            0 => write!(out, "free at {:#x}", row.start),
            _ => write!(out, "layer {} tile {}", row.at.0, row.at.1),
        };
    });
}

/// The columns MLOS adds -- what a page-based system could not say.
///
/// Empty strings where a column does not apply, so a consumer grouping by
/// tier or state never sees a bucket that is really "not an object".
pub fn extras<const N: usize>(
    out: &mut impl Write,
    manager: &Manager<'static, N>,
    base: u64,
    tail: Row,
) {
    let every = || rows(manager, base, tail);
    strs(out, "region_tier", every(), |row| row.tier);
    strs(out, "region_state", every(), |row| row.state);
    nums(out, "region_reuse", every(), |row| row.reuse);
    nums(out, "region_cost", every(), |row| row.cost);
    text(out, "region_object_id", every(), |out, row| {
        if row.object != 0 {
            let _ = write!(out, "{}", row.object);
        }
    });
    text(out, "region_next_use", every(), |out, row| {
        if row.object != 0 {
            let _ = write!(out, "{}", NextUseText(row.next_use));
        }
    });
}

/// The edge table: `backs`, from stored bytes to the arena region holding
/// them.
///
/// MLOS's version of SWTOS's catalog -> extent -> allocation chain, and
/// what an "explain this object" view consumes. Only weight tiles have
/// stored bytes: an activation is recomputed, so there is no disk extent
/// for an edge to run from, and drawing one would be a lie about where it
/// came from.
pub fn edges<const N: usize>(
    out: &mut impl Write,
    manager: &Manager<'static, N>,
    base: u64,
    tail: Row,
) {
    let every = || rows(manager, base, tail);
    /// The class nibble of a stable id.
    const TILE: u32 = ObjectClass::WeightTile as u32;
    let stored = |row: &Row| row.place == Where::Dram && (row.id >> 24) & 0xf == TILE;
    let backed = || every().filter(stored);
    let from = |row: &Row| u64::from(row.id & 0x0fff_ffff | (Where::Disk as u32) << 28);

    text(out, "rel_kind", backed(), |out, _| {
        let _ = out.write_str("backs");
    });
    nums(out, "rel_from", backed(), from);
    nums(out, "rel_to", backed(), |row| u64::from(row.id));
}
