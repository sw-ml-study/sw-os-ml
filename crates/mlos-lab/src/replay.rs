//! Replaying the recorded workload inside the kernel.
//!
//! The point of M3 step 009: `mlos-sim` and the kernel link the SAME
//! `mlos-policy` crate and replay the SAME trace under the same budget,
//! and the counts have to match exactly. A policy that behaves
//! differently in the two is a policy neither result describes, and the
//! simulator would have been measuring something that never ran.
//!
//! The trace arrives on the model disk, after the weights, because
//! `/chosen/bootargs` is far too small to carry one and the block
//! transport already exists. `mlos_synth::disk::TRACE_AT` is the sector
//! both sides agree on.
//!
//! Two static buffers, because the kernel has no allocator: one for the
//! text as it comes off the device and one for the parsed accesses.
//! `mlos-trace` parses into a caller-provided slice for exactly this
//! reason -- it was written in step 001 knowing this step was coming.

use mlos_abi::{Error, ObjectClass, ObjectId, Result};
use mlos_objman::Lease;
use mlos_objtab::SessionId;
use mlos_trace::parse;
use mlos_virtio_blk::SECTOR;

use crate::{state, with};

/// How much trace text the guest can hold.
pub const TEXT: usize = 256 * 1024;

/// How many accesses it can parse into.
pub const ACCESSES: usize = 12 * 1024;

/// How many DISTINCT objects a declaration may name.
///
/// Scratch for the next-use chain, and far smaller than `ACCESSES`
/// because that is the point: a decode loop touches a few hundred objects
/// thousands of times. Too small is refused rather than truncated.
pub const DISTINCT: usize = 2 * 1024;

/// What a replay cost, in the terms `mlos-sim` reports.
#[derive(Clone, Copy, Default)]
pub struct Replayed {
    /// Acquires that found the object already resident.
    pub hits: u64,
    /// Acquires that had to go to a provider.
    pub reads: u64,
    /// Bytes those reads moved.
    pub bytes: u64,
    /// Objects thrown away to make room.
    pub evicted: u64,
    /// Acquires that could not be served at all.
    pub refused: u64,
}

/// Reads the trace off the disk, declares it, and replays it.
///
/// Every counter comes from what the manager actually did rather than
/// from what the replay expected, which is the only way the comparison
/// means anything.
///
/// **The declaration is the whole trace, and that is not cheating.** The
/// kernel is being TOLD what it will acquire, which is exactly what
/// `ml_stream_declare` is for and exactly what a decode loop can honestly
/// say about itself. What it must not do is read the trace the way the
/// simulator does -- computing an answer from accesses nobody declared --
/// and it does not: `Stream` sees a sequence handed to it, and the cursor
/// advances one step per acquire like a running workload's would.
///
/// Declaring the trace rather than a 128-tile weight sweep is what closed
/// the last disagreement with the simulator. A sweep is periodic and a
/// decode loop is not: its KV cache accumulates, so every KV block read
/// as `Never` -- the most evictable thing in the table -- and next-use
/// spent 27 reads evicting exactly what it was about to want.
pub fn replay() -> Result<Replayed> {
    let room = state::replay_room();
    let held = read(room.text)?;
    let trace = parse(held, room.accesses).map_err(|_| Error::BadObject)?;
    let count = trace.accesses.len();
    let order = room.declared.get_mut(..count).ok_or(Error::NoBudget)?;
    for (slot, access) in order.iter_mut().zip(trace.accesses) {
        *slot = access.object;
    }
    mlos_stream::chain(order, room.next, room.seen)?;
    let declared: &'static [ObjectId] = room.declared;
    let next: &'static [u32] = room.next;
    with(|held| held.stream.declare(&declared[..count], next)).ok_or(Error::NoProvider)??;

    let mut out = Replayed::default();
    for access in trace.accesses {
        step(access.object, access.session, &mut out);
        with(|held| held.stream.advance(1));
    }
    Ok(out)
}

/// One access, and what it cost.
///
/// Every counter comes from what the manager actually DID -- the fault
/// count and the eviction count either side of the acquire -- rather than
/// from what the replay expected. That is the only way the comparison
/// means anything.
fn step(object: ObjectId, by: SessionId, out: &mut Replayed) {
    if !known(object) {
        out.refused += 1;
        return;
    }
    let before = with(|held| (held.counters.report().total_faults(), held.evictions));
    let Some((faults, evictions)) = before else {
        return;
    };
    let outcome = with(|held| held.acquire(object, Lease::Streaming, by));
    let after = with(|held| (held.counters.report().total_faults(), held.evictions));
    let Some((faults_now, evictions_now)) = after else {
        return;
    };

    out.evicted += evictions_now - evictions;
    match outcome {
        Some(Ok(handle)) if faults_now > faults => {
            out.reads += 1;
            out.bytes += u64::from(handle.size);
        }
        Some(Ok(_)) => out.hits += 1,
        _ => out.refused += 1,
    }
}

/// Makes sure the table has heard of `object`, registering it if not.
///
/// Not a convenience for the replay: a KV block comes into existence when
/// a session first writes one, and a kernel that could only serve objects
/// somebody had declared in advance could not run a decode loop at all.
/// `mlos-sim` behaves the same way for a different reason -- its `Model`
/// answers for any id -- and the two have to agree or the counts cannot.
fn known(object: ObjectId) -> bool {
    if with(|held| held.table.get(object).is_some()) == Some(true) {
        return true;
    }
    let meta = match object.class() {
        Some(ObjectClass::KvBlock) => mlos_synth::kv::meta(object.fields().model),
        Some(ObjectClass::Activation) => mlos_synth::model::activations(),
        _ => mlos_synth::model::weights(),
    };
    with(|held| held.register(object, meta)) == Some(Ok(()))
}

/// The trace text on the disk, as bytes.
///
/// An eight-byte length then the text, at the sector `mlos-synth` names.
/// A length longer than the buffer is refused rather than truncated: a
/// shortened trace is a different workload, and the counts it produced
/// would be confidently wrong rather than obviously absent.
fn read(into: &mut [u8]) -> Result<&str> {
    let disk = crate::devices::disk().ok_or(Error::NoProvider)?;
    let at = mlos_synth::disk::TRACE_AT * SECTOR as u64;
    // Sector-aligned reads only -- `Block::read_at` says so and refuses
    // otherwise, which is why the length is read as part of the first
    // sector rather than as eight bytes of its own and the text is
    // sliced out afterwards.
    let first = into.get_mut(..SECTOR).ok_or(Error::NoBudget)?;
    // SAFETY: the block device was brought up by `probe`, and this runs
    // on the boot core one request at a time.
    unsafe { disk.block.read_at(at, first) }?;

    let mut length = [0u8; 8];
    length.copy_from_slice(&first[..8]);
    let length = u64::from_le_bytes(length) as usize;
    let whole = (HEADER + length).next_multiple_of(SECTOR);
    let room = into.get_mut(..whole).ok_or(Error::NoBudget)?;
    // SAFETY: as above.
    unsafe { disk.block.read_at(at, room) }?;
    core::str::from_utf8(&room[HEADER..HEADER + length]).map_err(|_| Error::BadObject)
}

/// Bytes of length in front of the trace text.
const HEADER: usize = 8;
