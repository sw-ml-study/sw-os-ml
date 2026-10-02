//! The verbs that only look.
//!
//! Invariant: nothing here changes residency. Design: docs/notes/mlsh.md.

use core::fmt::Write;

use mlos_abi::ObjectClass;
use mlos_lab::{LAYERS, TILES};
use mlos_synth::model;

/// Lists the model's objects and what the table knows about each: tier,
/// residency, use count, next use.
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

/// Prints every tile the table knows, or only the resident ones. The
/// last column is `next_use`, `never` until something declares a stream.
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

/// The arena: what is in it, and what would still fit. Reports the
/// largest single run, not the total free, since after evictions they
/// differ; `Rm` is the same fact against the model, per mille.
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

/// The numbers `docs/PRD.md` s.5.2 says matter: `Rm`, `Ps`, `Ks`, `Ss`.
pub fn metrics(out: &mut impl Write) {
    let Some((report, h, sessions)) =
        mlos_lab::with(|m| (m.counters.report(), m.headline(), m.sessions.live()))
    else {
        return; // dispatch already said so
    };
    let rm = report.residency_per_mille().unwrap_or(0);
    let _ = writeln!(out, "  Rm       {rm}/1000 of the model resident");
    let _ = writeln!(
        out,
        "  Ps       {}/1000 weight bytes applied per byte read",
        h.ps_per_mille
    );
    let _ = writeln!(
        out,
        "  Ks       {} B of KV per session, {sessions} session(s)",
        h.ks
    );
    let _ = writeln!(
        out,
        "  Ss       {}/1000 sessions per GiB of arena",
        h.ss_milli
    );
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
