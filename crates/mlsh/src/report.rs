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
/// nothing, and the ones in memory are the ones a decision was made about.
fn list<const N: usize>(
    out: &mut impl Write,
    manager: &mlos_objman::Manager<'static, N>,
    all: bool,
) -> u32 {
    let mut shown = 0;
    for layer in 0..LAYERS {
        for tensor in 0..TILES {
            let Some(meta) = manager.table.get(model::tile(layer, tensor)) else {
                continue;
            };
            if meta.resident_at == 0 && !all {
                continue;
            }
            let _ = writeln!(
                out,
                "  L{layer:02} T{tensor:02}  {:?}  {:#x}  used {}",
                meta.tier, meta.resident_at, meta.reuse_count
            );
            shown += 1;
        }
    }
    shown
}

/// The arena: what is in it, and what would still fit.
pub fn arena(out: &mut impl Write) {
    let Some((used, size)) = mlos_lab::with(|manager| manager.arena.occupancy()) else {
        return; // dispatch already said so
    };
    let _ = writeln!(out, "  arena    {} of {} KiB used", used >> 10, size >> 10);
    let _ = writeln!(
        out,
        "  room for {} more tiles of {} B",
        (size - used) / u64::from(mlos_lab::TILE_BYTES),
        mlos_lab::TILE_BYTES
    );
    // Rm: the same fact against the model rather than the buffer, and the
    // ratio docs/PRD.md s.5.2 says the whole system optimises. A small
    // fraction of a large model resident is the good case, not a failure.
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
