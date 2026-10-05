//! The walk, applied to a manager: sessions moved down rungs, their KV
//! blocks rewritten in the table, and the lowest priority terminated.
//!
//! Invariant: a resident block is shrunk in place -- the arena gets the
//! tail back, the counters and the owner's account the difference -- so
//! a rung frees bytes without a read; a dropped block leaves the table.
//! Design: docs/notes/mlos-ladder.md.

use mlos_abi::{Error, ObjectClass, ObjectId, Result};
use mlos_objman::{Arena, Manager};
use mlos_session::Session;

use crate::{Fate, Move, Rung, Shape, movable, next};

/// What a squeeze did.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Squeezed {
    /// The deepest rung the walk reached: L6 once no session could go
    /// deeper and it still did not fit, L7 if anyone was terminated.
    pub reached: Rung,
    /// Bytes read to degrade: what `Rc` adds.
    pub recomputed: u64,
    /// Sessions terminated.
    pub terminated: u32,
    /// Whether the live sessions fit the budget at the end.
    pub fits: bool,
}

/// Sets the manager's budget to `bytes` and walks the ladder until the
/// live sessions fit it, or nothing more can be given up.
pub fn squeeze<const N: usize>(held: &mut Manager<'_, N>, shape: &Shape, bytes: u64) -> Squeezed {
    held.capacity.bytes = bytes;
    let mut done = Squeezed::default();
    loop {
        let reached = match next(&held.capacity, shape, &held.sessions) {
            Move::Fits => return Squeezed { fits: true, ..done },
            Move::Exhausted => Rung::L6,
            Move::Deepen(rung) => {
                done.recomputed += deepen(held, shape, rung);
                rung
            }
            Move::Terminate(id) => {
                let ended = held.destroy_session(id).is_ok();
                done.terminated += u32::from(ended);
                if ended { Rung::L7 } else { Rung::L6 }
            }
        };
        done.reached = done.reached.max(reached);
        if reached == Rung::L6 {
            return done;
        }
    }
}

/// Moves every session `rung` would move onto it. Returns the bytes read.
fn deepen<const N: usize>(held: &mut Manager<'_, N>, shape: &Shape, rung: Rung) -> u64 {
    let mut read = 0;
    for slot in 0..mlos_session::MAX_SESSIONS {
        match held.sessions.at(slot).copied() {
            Some(s) if movable(&s, rung) => read += step(held, shape, s, rung),
            _ => {}
        }
    }
    read
}

/// Moves session `s` to `to`: rewrites each of its KV blocks to what
/// `to` makes of it and records the rung, the coarsest precision left,
/// and the bytes read. Returns those bytes.
fn step<const N: usize>(held: &mut Manager<'_, N>, shape: &Shape, s: Session, to: Rung) -> u64 {
    let (blocks, mut coarsest) = (shape.blocks(s.contract.context), s.delivered.coarsest);
    for slot in 0..N {
        if let Some((object, meta)) = held.table.at(slot)
            && meta.owner == s.id
            && object.class() == Some(ObjectClass::KvBlock)
        {
            let fate = shape.fate(to, u32::from(object.fields().tensor), blocks);
            if let (Ok(()), Some(p)) = (rewrite(held, object, fate), fate.precision())
                && p as u8 > coarsest as u8
            {
                coarsest = p;
            }
        }
    }
    let read = shape.cost(s.rung, to, s.contract.context, held.capacity.kv_per_token);
    if let Some(live) = held.sessions.get_mut(s.id) {
        (live.rung, live.delivered.coarsest) = (to, coarsest);
        live.delivered.deepest = live.delivered.deepest.max(to);
        live.delivered.recomputed = live.delivered.recomputed.saturating_add(read);
    }
    read
}

/// Makes `object` what `fate` says: gone, or smaller and coarser, in
/// place if resident. A release the arena refuses changes nothing.
fn rewrite<const N: usize>(held: &mut Manager<'_, N>, object: ObjectId, fate: Fate) -> Result<()> {
    let meta = *held.table.get(object).ok_or(Error::BadObject)?;
    let Some(precision) = fate.precision() else {
        let evicted = if meta.resident_at == 0 {
            Ok(0)
        } else {
            held.evict(object)
        };
        return evicted.map(|_| _ = held.table.remove(object));
    };
    // Back to full precision first: a Q4 block of 64 bytes was 256.
    let full = 16 * u64::from(meta.size) / Fate::Keep(meta.precision).bytes(16).max(1);
    let size = u32::try_from(fate.bytes(full)).map_or(meta.size, |s| s.min(meta.size));
    let [old, new] = [meta.size, size].map(|n| n.next_multiple_of(Arena::ALIGN));
    if meta.resident_at != 0 && new < old {
        held.arena
            .release(meta.resident_at + u64::from(new), old - new)?;
        held.counters
            .held(ObjectClass::KvBlock, -i64::from(meta.size - size));
        held.sessions.credit(meta.owner, meta.size - size);
    }
    let changed = held.table.get_mut(object).ok_or(Error::BadObject)?;
    (changed.size, changed.precision) = (size, precision);
    Ok(())
}
