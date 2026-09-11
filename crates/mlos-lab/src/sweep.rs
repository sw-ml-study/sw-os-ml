//! Walking the model in order.
//!
//! In order, because that is what a dense transformer does, and the whole
//! argument of `docs/PRD.md` is that the order is knowable in advance.
//! Nothing here exploits that yet -- exploiting it is M3 -- but this is
//! the sweep whose numbers M3 has to improve on.

use mlos_objman::{Lease, Manager};
use mlos_objtab::SessionId;
use mlos_synth::{LAYERS, TILES, model};

use crate::{CAPACITY, manager};

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
            if let Err(error) = held.acquire(id, Lease::Streaming, SessionId(session)) {
                return (acquired, Some(error));
            }
            acquired += 1;
        }
    }
    (acquired, None)
}
