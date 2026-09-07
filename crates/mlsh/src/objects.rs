//! The verbs that are about ML objects.
//!
//! Separate from `commands` because this is the part `mlsh` exists for.
//! Everything else it can tell you -- the memory map, the devices -- any
//! small operating system could. These three are about the object table,
//! which is the thing MLOS has that others do not.

use core::fmt::Write;

use mlos_abi::ObjectClass;

/// Registers the synthetic model.
pub fn model(out: &mut impl Write) {
    match mlos_synth::register() {
        Ok((objects, bytes)) => {
            let _ = writeln!(
                out,
                "  registered {objects} objects, {} KiB across 3 tiers",
                bytes >> 10
            );
            let _ = writeln!(
                out,
                "  {} layers x {} tiles of {} B, plus one activation each",
                mlos_synth::LAYERS,
                mlos_synth::TILES,
                mlos_synth::TILE_BYTES
            );
        }
        Err(error) => {
            let _ = writeln!(out, "  could not register: {error:?}");
        }
    }
}

/// Sweeps the model, faulting every tile in.
pub fn sweep(out: &mut impl Write) {
    let swept = mlos_synth::sweep(1);
    let _ = writeln!(
        out,
        "  acquired {} of {} tiles",
        swept.acquired, swept.total
    );
    match swept.stopped {
        // Not a failure. The arena is a quarter of the model, nothing
        // evicts yet, and running out is the honest outcome -- it is the
        // problem M3 exists to solve.
        Some(error) => {
            let _ = writeln!(out, "  stopped: {error:?} -- no eviction policy yet");
        }
        None => {
            let _ = writeln!(out, "  swept the whole model");
        }
    }
}

/// The per-class breakdown: the line that says what the system is
/// actually struggling with, which a total never answers.
fn by_class(out: &mut impl Write, report: &mlos_metrics::Report) {
    for class in ObjectClass::ALL {
        let count = report.faults[class.index()];
        if count > 0 {
            let fetched = report.fetched[class.index()] >> 10;
            let _ = writeln!(out, "    {class:?}: {count} faults, {fetched} KiB fetched");
        }
    }
    let resident = report.residency_per_mille().unwrap_or(0);
    let _ = writeln!(
        out,
        "  Rm       {}/{} KiB resident = {resident} per mille",
        report.resident >> 10,
        report.registered >> 10
    );
}

/// Reports what it all cost.
pub fn faults(out: &mut impl Write) {
    let Some((report, last)) = mlos_synth::report() else {
        let _ = writeln!(out, "  no model registered (try `model`)");
        return;
    };
    let _ = writeln!(out, "  faults   {} total", report.total_faults());
    by_class(out, &report);

    if let Some(fault) = last {
        let _ = writeln!(
            out,
            "  last     {:?} model {} layer {} tensor {} tile {}, {} us",
            fault.class,
            fault.model,
            fault.layer,
            fault.tensor,
            fault.tile,
            fault.cost.0 / 1000
        );
    }
}
