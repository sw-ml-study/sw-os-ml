//! Acquiring one object by hand: `get` and `evict`, the verbs that make
//! the fault path pokeable.
//!
//! Invariant: residency is read before the acquire, since afterwards the
//! object is resident either way. Design: docs/notes/mlsh.md.

use core::fmt::Write;

use mlos_objman::Lease;
use mlos_objtab::SessionId;
use mlos_synth::model;

/// Throws one tile out, and says what came back.
pub fn evict(out: &mut impl Write, args: &str) {
    let mut numbers = args.split_whitespace().filter_map(|n| n.parse().ok());
    let Some((layer, tensor)) = numbers.next().zip(numbers.next()) else {
        let _ = writeln!(out, "  usage: evict LAYER TILE");
        return;
    };
    let id = model::tile(layer, tensor);
    match mlos_lab::with(|held| held.evict(id)) {
        Some(Ok(bytes)) => drop(writeln!(out, "  evicted {bytes} B, returned to the arena")),
        Some(Err(why)) => drop(writeln!(out, "  not evicted: {why:?}")),
        None => {}
    }
}

/// Acquires one tile by hand, and says whether it had to fault.
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

/// Whether it hit or faulted, where it landed, and its first byte: the
/// disk image sets a high nibble the stub tier never writes, so the byte
/// says where the bytes came from.
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

/// Acquires one tile, reporting whether it was already resident. The
/// residency is read before the acquire; afterwards the fact is gone.
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
