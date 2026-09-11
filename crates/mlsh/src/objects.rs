//! The verbs that make the object manager do something.
//!
//! Separate from `report`, which only looks. The distinction matters more
//! here than it usually would: `sweep` and `get` change residency, so
//! running one changes what the other reports, and knowing which is which
//! is the difference between exploring a system and disturbing it.

use core::fmt::Write;

use mlos_objman::Lease;
use mlos_objtab::SessionId;
use mlos_synth::model;

/// Default arena, in bytes: a quarter of the model's weights.
const DEFAULT_BUDGET: usize = 32 * 1024;

/// Registers the synthetic model, optionally with a different budget.
///
/// `model` for the default, `model 8` for eight kibibytes -- which is the
/// knob worth having, because the whole subject is what happens when
/// memory is smaller than the model.
pub fn model(out: &mut impl Write, args: &str) {
    let budget = args
        .split_whitespace()
        .next()
        .and_then(|kib| kib.parse::<usize>().ok())
        .map_or(DEFAULT_BUDGET, |kib| kib * 1024);

    match mlos_lab::register(budget) {
        Ok((objects, bytes)) => registered(out, objects, bytes, budget),
        Err(error) => {
            let _ = writeln!(out, "  could not register: {error:?}");
        }
    }
}

/// What was registered, and where its bytes will come from.
fn registered(out: &mut impl Write, objects: u32, bytes: u64, budget: usize) {
    let _ = writeln!(
        out,
        "  registered {objects} objects, {} KiB across 3 tiers",
        bytes >> 10
    );
    let _ = writeln!(out, "  budget     {} KiB of arena", budget >> 10);
    let source = if mlos_lab::on_disk() {
        "virtio-blk"
    } else {
        "a pattern (no disk attached)"
    };
    let _ = writeln!(out, "  weights    from {source}");
}

/// Sweeps the model, faulting every tile in.
pub fn sweep(out: &mut impl Write) {
    let swept = mlos_lab::sweep(1);
    let _ = writeln!(
        out,
        "  acquired {} of {} tiles",
        swept.acquired, swept.total
    );
    match swept.stopped {
        // Not a failure. The arena is smaller than the model, nothing
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

/// Acquires one tile by hand, and says whether it had to fault.
///
/// The verb that makes the fault path pokeable. Running it twice on the
/// same tile is the shortest possible demonstration of what the object
/// table is for: the second time costs nothing.
pub fn get(out: &mut impl Write, args: &str) {
    let mut numbers = args
        .split_whitespace()
        .filter_map(|word| word.parse::<u16>().ok());
    let (Some(layer), Some(tensor)) = (numbers.next(), numbers.next()) else {
        let _ = writeln!(out, "  usage: get <layer> <tensor>");
        return;
    };
    match acquire(layer, tensor) {
        None => {} // dispatch already said so
        Some((_, Err(error), _)) => {
            let _ = writeln!(out, "  refused: {error:?}");
        }
        Some((resident, Ok(handle), fault)) => outcome(out, resident, &handle, fault),
    }
}

/// Whether it hit or faulted, where it landed, and what it holds.
///
/// The first byte is printed because it is the cheapest possible proof of
/// provenance: the stub tier fills with `layer ^ tensor`, and the disk
/// image sets a high nibble the stub never writes.
fn outcome(
    out: &mut impl Write,
    resident: bool,
    handle: &mlos_objman::Handle,
    fault: Option<mlos_objman::ModelFault>,
) {
    let how = if resident { "hit" } else { "faulted" };
    // SAFETY: the handle names memory the arena owns and the lease is
    // still held, which is exactly when an address from a handle is valid.
    let first = unsafe { core::ptr::read_volatile(handle.address as *const u8) };
    let _ = writeln!(
        out,
        "  {how}: {} B at {:#x}, first byte {first:#04x}",
        handle.size, handle.address
    );
    if let (false, Some(fault)) = (resident, fault) {
        let _ = writeln!(out, "  cost {} us", fault.cost.0 / 1000);
    }
}

/// Acquires one tile, reporting whether it was already resident.
///
/// The residency is read *before* the acquire, because afterwards every
/// object is resident and the interesting fact -- whether this one had to
/// be fetched -- is gone.
type Acquired = (
    bool,
    mlos_abi::Result<mlos_objman::Handle>,
    Option<mlos_objman::ModelFault>,
);
fn acquire(layer: u16, tensor: u16) -> Option<Acquired> {
    let id = model::tile(layer, tensor);
    mlos_lab::with(|manager| {
        let resident = manager
            .table
            .get(id)
            .is_some_and(|meta| meta.resident_at != 0);
        let acquired = manager.acquire(id, Lease::Pin, SessionId(1));
        (resident, acquired, manager.last_fault)
    })
}
