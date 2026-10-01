//! Walking the model in order, and declaring that order as a stream.
//!
//! Invariant: a declaration is one pass, not a repeating one; a tile
//! behind the cursor reads `Never`. Design and history:
//! docs/notes/mlos-lab.md.

use mlos_abi::ObjectId;
use mlos_objman::Manager;
use mlos_objtab::SessionId;
use mlos_synth::{LAYERS, TILES, model};

use crate::{CAPACITY, manager, with};

/// How a sweep ended.
pub struct Swept {
    /// Tiles acquired before something stopped it.
    pub acquired: u32,
    /// Tiles the model has in total.
    pub total: u32,
    /// Why it stopped, if it did.
    pub stopped: Option<mlos_abi::Error>,
}
/// Walks every tile in order, acquiring each one.
pub fn sweep(session: u16) -> Swept {
    let total = u32::from(LAYERS) * u32::from(TILES);
    let Some(held) = manager().as_mut() else {
        return Swept {
            acquired: 0,
            total,
            stopped: Some(mlos_abi::Error::NoProvider),
        };
    };
    let (acquired, stopped) = walk(held, session);
    Swept {
        acquired,
        total,
        stopped,
    }
}

/// Acquires tiles in order until one refuses.
fn walk(held: &mut Manager<'static, CAPACITY>, session: u16) -> (u32, Option<mlos_abi::Error>) {
    let mut acquired = 0;
    for layer in 0..LAYERS {
        for tensor in 0..TILES {
            let id = model::tile(layer, tensor);
            if let Err(error) = held.consume(id, SessionId(session)) {
                return (acquired, Some(error));
            }
            acquired += 1;
        }
    }
    (acquired, None)
}

/// Declares one sweep of this model, in the order it performs it, into
/// the replay's declaration buffers. Returns how many objects were
/// declared, or `None` if there is no manager or no room.
pub fn declare() -> Option<usize> {
    let room = crate::state::replay_room();
    let count = (LAYERS * TILES) as usize;
    let order = room.declared.get_mut(..count)?;
    for layer in 0..LAYERS {
        for tensor in 0..TILES {
            order[(layer * TILES + tensor) as usize] = model::tile(layer, tensor);
        }
    }
    mlos_stream::chain(order, room.next, room.seen).ok()?;
    let (declared, next): (&'static [ObjectId], &'static [u32]) = (room.declared, room.next);
    with(|held| {
        held.stream
            .declare(&declared[..count], next)
            .ok()
            .map(|()| count)
    })?
}

/// Moves the declared stream on by `steps`, returning the new cursor.
pub fn advance(steps: u32) -> Option<u32> {
    with(|held| {
        held.stream.advance(steps);
        held.stream.cursor()
    })
}
