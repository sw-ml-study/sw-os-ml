//! Replaying the recorded workload inside the kernel, so its counts can
//! be compared with `mlos-sim`'s exactly.
//!
//! Invariant: every counter comes from what the manager did, never from
//! what the replay expected; a trace that does not fit is refused, not
//! truncated. Design and history: docs/notes/mlos-lab.md.

use mlos_abi::{Error, ObjectClass, ObjectId, Result};
use mlos_metrics::Headline;
use mlos_objman::{Contract, Session};
use mlos_objtab::SessionId;
use mlos_sched::{ParameterMajor, ProcessMajor, Schedule};
use mlos_trace::parse;
use mlos_virtio_blk::SECTOR;

use crate::lanes::{Lane, Lanes};
use crate::{state, with};

/// How much trace text the guest can hold.
pub const TEXT: usize = 256 * 1024;

/// How many accesses it can parse into.
pub const ACCESSES: usize = 12 * 1024;

/// How many distinct objects a declaration may name. Too small is
/// refused, not truncated.
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
    /// `Ps`, `Ks`, `Ss` at the end of the replay, as `mlos-metrics` computes them.
    pub headline: Headline,
}

/// Reads the trace off the disk, orders it parameter-major or
/// process-major over the sessions it names, declares that order as the
/// stream, and replays it one acquire per step.
pub fn replay(parameter: bool) -> Result<Replayed> {
    let (mut process, mut lockstep) = (ProcessMajor::default(), ParameterMajor);
    let schedule: &mut dyn Schedule = if parameter {
        &mut lockstep
    } else {
        &mut process
    };
    let mut room = state::replay_room();
    let count = prepare(&mut room, schedule)?;
    let (declared, next): (&'static [ObjectId], &'static [u32]) = (room.declared, room.next);
    with(|held| held.stream.declare(&declared[..count], next)).ok_or(Error::NoProvider)??;
    let mut out = Replayed::default();
    for (object, by) in declared[..count].iter().zip(&room.sessions[..count]) {
        step(*object, *by, &mut out);
        with(|held| held.stream.advance(1));
    }
    out.headline = with(|held| held.headline()).unwrap_or_default();
    with(|held| held.destroy_all_sessions());
    Ok(out)
}

/// Parses the disk trace, adopts its sessions, cuts it into lanes, and
/// writes the order `schedule` chooses into the room, chained for
/// next-use. Returns how many accesses there are.
fn prepare(room: &mut state::Room, schedule: &mut dyn Schedule) -> Result<usize> {
    let held = read(room.text)?;
    let trace = parse(held, room.accesses).map_err(|_| Error::BadObject)?;
    let named = trace.accesses.iter().map(|access| access.session);
    with(|held| held.sessions.adopt_each(named, Contract::NONE));
    // One lane per live session, in slot order: the order the trace first
    // named them in.
    let lanes = &mut *room.lanes;
    let name = |(lane, s): (&mut Lane, &Session)| *lane = (s.id, 0, 0, s.contract.latency_ceiling);
    let count =
        with(|held| lanes.iter_mut().zip(held.sessions.each()).map(name).count()).unwrap_or(0);
    let first = mlos_synth::model::tile(0, 0);
    let buffers = (&mut *room.grouped, &mut *room.shape, lanes);
    let mut lanes = Lanes::build(trace.accesses, first, buffers, count)?;
    let ordered = lanes.order(schedule, (&mut *room.declared, &mut *room.sessions));
    if ordered != trace.accesses.len() {
        return Err(Error::NoBudget);
    }
    mlos_stream::chain(&room.declared[..ordered], &mut *room.next, &mut *room.seen)?;
    Ok(ordered)
}

/// One access, and what it cost, measured by differencing the manager's
/// own fault and eviction counts around the acquire.
fn step(object: ObjectId, by: SessionId, out: &mut Replayed) {
    if !known(object) {
        out.refused += 1;
        return;
    }
    let before = with(|held| (held.counters.report().total_faults(), held.evictions));
    let Some((faults, evictions)) = before else {
        return;
    };
    let outcome = with(|held| held.consume(object, by));
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

/// Makes sure the table has heard of `object`, registering it if not, as
/// `mlos-sim`'s `Model` does for any id.
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

/// The trace text on the disk: an eight-byte length then the text, at the
/// sector `mlos-synth` names. A length longer than `into` is refused.
fn read(into: &mut [u8]) -> Result<&str> {
    let disk = crate::devices::disk().ok_or(Error::NoProvider)?;
    let at = mlos_synth::disk::TRACE_AT * SECTOR as u64;
    // `Block::read_at` refuses anything but sector-aligned reads, so the
    // length comes in with the first sector.
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
