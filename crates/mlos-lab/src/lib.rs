//! The one live object manager.
//!
//! `mlos-synth` says what a synthetic model is; this holds one, in an
//! arena, reading from whatever device this machine turned out to have.
//! It is what makes `docs/PRD.md` gates G2 and G3 observable rather than
//! merely tested.
//!
//! The arena is deliberately smaller than the model. That is not a
//! limitation to apologise for -- it is the entire premise. RAM is the
//! scarce resource, the model does not fit, and what the system does
//! about that is the subject.

#![no_std]

mod devices;
mod state;
mod sweep;

use mlos_abi::Result;
use mlos_objman::{Arena, Manager};
use mlos_synth::{disk, model, tiers};

use state::{arena, manager};

pub use devices::{on_disk, set_slots};
pub use mlos_synth::{ACTIVATION_BYTES, LAYERS, TILE_BYTES, TILES};
pub use sweep::{Swept, sweep};

/// Objects the table can hold. Comfortably more than the model needs, so
/// a full table is never what a sweep runs into first.
pub(crate) const CAPACITY: usize = 512;

/// Bytes the static arena holds; the ceiling on any budget.
///
/// A quarter of the model's weights, chosen so a sweep runs out. A
/// demonstration where everything fits demonstrates nothing: the
/// interesting number is how far it got.
pub const ARENA_BYTES: usize = 32 * 1024;

/// Registers the model, replacing whatever was there.
///
/// Returns how many objects were registered and how many bytes they are.
pub fn register(budget: usize) -> Result<(u32, u64)> {
    let manager = manager();
    let bytes = arena();
    // Clamped: a budget below one tile can hold nothing, and one above
    // the static buffer does not exist. Both are user input.
    let limit = budget.clamp(TILE_BYTES as usize, bytes.len());
    *manager = Some(Manager::new(Arena::new(&mut bytes[..limit])));

    let held = manager.as_mut().ok_or(mlos_abi::Error::NoProvider)?;
    held.attach(&tiers::BACKING)?;
    held.attach(&tiers::RECOMPUTE)?;
    // A disk if the machine has one, and the stub otherwise. The model is
    // registered against whichever is present, so the same commands work
    // either way and the difference shows up only in where bytes came from.
    if let Some(disk) = devices::disk() {
        held.attach(disk)?;
    }
    let objects = populate(held, devices::on_disk())?;
    Ok((objects, held.counters.report().registered))
}

/// Registers every tile and activation, returning how many there were.
fn populate(held: &mut Manager<'static, CAPACITY>, on_disk: bool) -> Result<u32> {
    let mut objects = 0;
    for layer in 0..LAYERS {
        for tensor in 0..TILES {
            let mut meta = model::weights();
            if on_disk {
                meta.provider = disk::DISK;
            }
            held.register(model::tile(layer, tensor), meta)?;
            objects += 1;
        }
        held.register(model::activation(layer), model::activations())?;
        objects += 1;
    }
    Ok(objects)
}

/// Runs `visit` against the manager, if there is one.
///
/// One accessor rather than a wrapper for every question. The shell wants
/// to ask things nobody has thought of yet -- what tier is this in, how
/// often has it been used, what would evicting it save -- and a crate
/// that answers only the questions it anticipated is one the shell has to
/// be extended through every time it wants a new one.
pub fn with<T>(visit: impl FnOnce(&mut Manager<'static, CAPACITY>) -> T) -> Option<T> {
    manager().as_mut().map(visit)
}
