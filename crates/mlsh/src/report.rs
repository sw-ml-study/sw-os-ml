//! The verbs that only look.
//!
//! Nothing here changes residency, which is what makes them safe to run
//! while working out what the acting verbs did.

use core::fmt::Write;

use mlos_abi::ObjectClass;
use mlos_lab::{LAYERS, TILES};
use mlos_synth::model;

/// Lists the model's objects and what the table knows about each.
///
/// The inspector `docs/design.md` s.9 asked for. A page table would have
/// nothing worth listing -- present, dirty, accessed. This has a tier, a
/// residency, and how often each object has been wanted, which is the
/// state every policy in `docs/PRD.md` reads.
pub fn objs(out: &mut impl Write, args: &str) {
    let all = args.split_whitespace().next() == Some("all");
    match mlos_lab::with(|manager| list(out, manager, all)) {
        Some(0) => {
            let _ = writeln!(out, "  nothing resident (try `sweep`, or `objs all`)");
        }
        Some(shown) => {
            let _ = writeln!(out, "  {shown} shown");
        }
        None => {} // dispatch already said so
    }
}

/// Prints every tile the table knows, or only the resident ones.
///
/// Resident by default: a full listing of 128 identical cold tiles says
/// nothing, and the ones in memory are the ones a decision was made
/// about.
///
/// The last column is `next_use` -- when the object is next wanted,
/// which no page-based system can hold. It reads `never` until something
/// declares a stream, because until M3 step 007 nothing ever wrote it.
fn list<const N: usize>(
    out: &mut impl Write,
    manager: &mlos_objman::Manager<'static, N>,
    all: bool,
) -> u32 {
    let tiles = (0..LAYERS).flat_map(|layer| (0..TILES).map(move |tensor| (layer, tensor)));
    let mut shown = 0;
    for (layer, tensor) in tiles {
        let Some(meta) = manager.table.get(model::tile(layer, tensor)) else {
            continue;
        };
        if meta.resident_at == 0 && !all {
            continue;
        }
        let next = mlos_spaces::NextUseText(meta.next_use);
        let (tier, at, used) = (meta.tier, meta.resident_at, meta.reuse_count);
        let _ = writeln!(
            out,
            "  L{layer:02} T{tensor:02}  {tier:?}  {at:#x}  used {used}  next {next}"
        );
        shown += 1;
    }
    shown
}

/// The arena: what is in it, and what would still fit.
///
/// The largest single RUN, not the total free. After evictions those
/// differ, and the difference is memory the arena holds and cannot give
/// to anything -- a policy evicting perfectly into a fragmented arena has
/// not helped.
///
/// `Rm` is the same fact against the model rather than the buffer, and
/// the ratio `docs/PRD.md` s.5.2 says the whole system optimises. A small
/// fraction of a large model resident is the good case, not a failure.
pub fn arena(out: &mut impl Write) {
    let Some(arena) = mlos_lab::with(|manager| manager.arena.occupancy()) else {
        return; // dispatch already said so
    };
    let (used, size) = (arena.used, arena.capacity);
    let _ = writeln!(out, "  arena    {} of {} KiB used", used >> 10, size >> 10);
    let tile = u64::from(mlos_lab::TILE_BYTES);
    let (room, run) = (arena.largest / tile, arena.largest);
    let _ = writeln!(
        out,
        "  room for {room} more tiles of {tile} B, largest run {run} B"
    );
    if let Some((report, _)) = mlos_lab::with(|m| (m.counters.report(), ())) {
        let rm = report.residency_per_mille().unwrap_or(0);
        let _ = writeln!(
            out,
            "  Rm       {}/{} KiB of the model = {rm} per mille",
            report.resident >> 10,
            report.registered >> 10
        );
    }
}

/// What it all cost.
pub fn faults(out: &mut impl Write) {
    let Some((report, last)) = mlos_lab::with(|m| (m.counters.report(), m.last_fault)) else {
        return; // dispatch already said so
    };
    let _ = writeln!(out, "  faults   {} total", report.total_faults());
    for class in ObjectClass::ALL {
        let count = report.faults[class.index()];
        if count > 0 {
            let fetched = report.fetched[class.index()] >> 10;
            let _ = writeln!(out, "    {class:?}: {count} faults, {fetched} KiB fetched");
        }
    }

    // Only one model exists, so its number is noise; the layer and
    // tensor are what a page fault could never have told you.
    if let Some(f) = last {
        let us = f.cost.0 / 1000;
        let (class, tile) = (f.class, f.tile);
        let _ = writeln!(
            out,
            "  last     {class:?} L{} T{} tile {tile}, {us} us",
            f.layer, f.tensor
        );
    }
}
