//! Getting on the ladder, and the walk's next move down it.
//!
//! Invariant: every session is deepened one rung at a time, all
//! together, never to a rung its quality floor does not permit and never
//! onto L6 or L7; a session is terminated only when no session can go
//! deeper. Design: docs/notes/mlos-ladder.md.

use mlos_admit::Capacity;
use mlos_objman::{Contract, Manager, Refusal};
use mlos_objtab::ObjectMeta;
use mlos_session::{Session, SessionId, Sessions};
use mlos_synth::kv::block;

use crate::{Rung, Shape};

/// The next thing the ladder does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Move {
    /// The live sessions fit the budget as they are.
    Fits,
    /// Put every session that may go there on this rung.
    Deepen(Rung),
    /// No session may go deeper: L6 is in force, and this one, the
    /// lowest priority holding any context, goes (L7).
    Terminate(SessionId),
    /// Nothing is left to give up and it still does not fit: the
    /// reserve alone is over the budget.
    Exhausted,
}

/// Whether `session` is one `rung` would move: it holds a declared
/// context, it is shallower, `rung` is one a session can be on (L5 or
/// above), and its contract's floor permits the rung's quality.
#[must_use]
pub fn movable(session: &Session, rung: Rung) -> bool {
    let contract = &session.contract;
    contract.context != 0
        && session.rung < rung
        && rung <= Rung::L5
        && contract.permits(rung.quality())
}

/// The ladder's next move for `live` under `capacity`. Lowest priority
/// is the highest id: MLOS has no priority field, and the highest id is
/// the latest promise among those still live when ids are not reused.
#[must_use]
pub fn next<const N: usize>(capacity: &Capacity, shape: &Shape, live: &Sessions<N>) -> Move {
    if fits(capacity, shape, live) {
        return Move::Fits;
    }
    let deeper = live
        .each()
        .filter_map(|s| s.rung.deeper().filter(|r| movable(s, *r)))
        .min();
    if let Some(rung) = deeper {
        return Move::Deepen(rung);
    }
    live.each()
        .filter(|s| s.contract.context != 0)
        .map(|s| s.id)
        .max_by_key(|id| id.0)
        .map_or(Move::Exhausted, Move::Terminate)
}

/// Whether what `live` holds on its rungs fits what the budget has left
/// after the reserve. A budget of zero bytes fits anything, as in
/// admission.
fn fits<const N: usize>(capacity: &Capacity, shape: &Shape, live: &Sessions<N>) -> bool {
    let demand = live
        .each()
        .map(|s| shape.need(s.rung, s.contract.context, capacity.kv_per_token))
        .fold(0u64, u64::saturating_add);
    capacity.bytes == 0 || demand <= capacity.bytes.saturating_sub(capacity.reserved)
}

/// `ml_session_create` for a session that arrives with its context
/// already cached: admits `contract`, then registers and places a block
/// per layer per `shape` block of its context, `template.size` bytes per
/// token, owned by it. Returns the session and how many blocks were
/// placed before anything refused; a short prefill is not a refusal.
pub fn arrive<const N: usize>(
    held: &mut Manager<'_, N>,
    shape: &Shape,
    contract: Contract,
    (layers, template): (u16, ObjectMeta),
) -> Result<(SessionId, u32), Refusal> {
    let (id, context) = (held.create_session(contract)?, contract.context);
    let mut meta = template;
    meta.owner = id;
    let mut placed = 0;
    for at in 0..shape.blocks(context) {
        let full = shape.full(at, context, u64::from(template.size));
        meta.size = u32::try_from(full).unwrap_or(u32::MAX);
        for object in (0..layers).map(|layer| block(id.0, layer, at as u16)) {
            match held.register(object, meta) {
                Ok(()) if held.consume(object, id).is_ok() => placed += 1,
                _ => return Ok((id, placed)),
            }
        }
    }
    Ok((id, placed))
}
