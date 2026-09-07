//! A synthetic model, and the manager that holds it.
//!
//! This is what makes `docs/PRD.md` gates G2 and G3 observable rather
//! than merely tested: a model registered across three tiers, a sweep
//! that faults its way through, and counters that say what it cost.
//!
//! The arena is deliberately smaller than the model. That is not a
//! limitation to apologise for -- it is the entire premise. RAM is the
//! scarce resource, the model does not fit, and what the system does
//! about that is the subject.

#![no_std]

mod model;
mod state;
mod tiers;

use mlos_abi::Result;
use mlos_metrics::Report;
use mlos_objman::{Arena, Lease, Manager};

use mlos_objtab::SessionId;
use state::{arena, manager};

pub use model::{ACTIVATION_BYTES, LAYERS, TILE_BYTES, TILES};

/// Objects the table can hold. Comfortably more than the model needs, so
/// a full table is never what a sweep runs into first.
pub(crate) const CAPACITY: usize = 512;

/// Bytes the arena holds.
///
/// A quarter of the model's weights, chosen so a sweep runs out. A
/// demonstration where everything fits demonstrates nothing: the
/// interesting number is how far it got.
pub(crate) const ARENA_BYTES: usize = 32 * 1024;

/// How a sweep ended.
pub struct Swept {
    /// Tiles acquired before something stopped it.
    pub acquired: u32,
    /// Tiles the model has in total.
    pub total: u32,
    /// Why it stopped, if it did.
    pub stopped: Option<mlos_abi::Error>,
}

/// Registers the model, replacing whatever was there.
///
/// Returns how many objects were registered and how many bytes they are.
pub fn register() -> Result<(u32, u64)> {
    let manager = manager();
    *manager = Some(Manager::new(Arena::new(arena())));
    let held = manager.as_mut().ok_or(mlos_abi::Error::NoProvider)?;
    held.attach(&tiers::BACKING)?;
    held.attach(&tiers::RECOMPUTE)?;

    let mut objects = 0;
    for layer in 0..LAYERS {
        for tensor in 0..TILES {
            held.register(model::tile(layer, tensor), model::weights())?;
            objects += 1;
        }
        held.register(model::activation(layer), model::activations())?;
        objects += 1;
    }
    Ok((objects, held.counters.report().registered))
}

/// Walks every tile in order, acquiring each one.
///
/// In order, because that is what a dense transformer does, and the whole
/// argument of `docs/PRD.md` is that the order is knowable in advance.
/// Nothing here exploits that yet -- exploiting it is M3 -- but this is
/// the sweep whose numbers M3 has to improve on.
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

/// What has happened so far, and where the last fault was.
#[must_use]
pub fn report() -> Option<(Report, Option<mlos_objman::ModelFault>)> {
    let held = manager().as_ref()?;
    Some((held.counters.report(), held.last_fault))
}
