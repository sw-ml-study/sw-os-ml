//! The one live object manager: a synthetic model in a static arena,
//! driven from the shell.
//!
//! Invariant: the arena is smaller than the model; that is the premise,
//! not a limit. Design and history: docs/notes/mlos-lab.md.

#![no_std]

mod devices;
mod lanes;
mod replay;
mod state;
mod sweep;

use mlos_abi::Result;
use mlos_admit::Capacity;
use mlos_objman::{Arena, Manager};
use mlos_synth::{disk, model, tiers};

use state::{arena, manager};

pub use devices::{on_disk, set_slots};
pub use mlos_synth::{ACTIVATION_BYTES, LAYERS, TILE_BYTES, TILES};
pub use replay::{Replayed, replay};
pub use sweep::{Swept, advance, declare, sweep};

/// Objects the table can hold; more than the model needs.
pub(crate) const CAPACITY: usize = 512;

/// Bytes the static arena holds; the ceiling on any budget. A quarter of
/// the model's weights, so a sweep runs out.
pub const ARENA_BYTES: usize = 32 * 1024;

/// What the synthetic model asks of a budget, less the budget itself:
/// one layer's tiles and activation reserved as the per-token working
/// set, a KV block per layer per token of context, and a token of every
/// tile once. `register` fills in the bytes.
pub const SYNTHETIC: Capacity = Capacity {
    bytes: 0,
    reserved: TILES as u64 * TILE_BYTES as u64 + ACTIVATION_BYTES as u64,
    kv_per_token: mlos_synth::kv::BYTES as u64 * LAYERS as u64,
    token: LAYERS as u32 * TILES as u32,
};

/// Which policy the kernel evicts with, by name; `false` if the name is
/// unknown or there is no manager. `demand` is no policy at all.
pub fn choose(name: &str) -> bool {
    let policy: Option<&'static dyn mlos_policy::Policy> = match name {
        "fifo" => Some(&mlos_policy::FIFO),
        "lru" => Some(&mlos_policy::LRU),
        "next-use" | "nextuse" => Some(&mlos_policy::NEXT_USE),
        "demand" => None,
        _ => return false,
    };
    with(|held| held.policy = policy).is_some()
}

/// Registers the model, replacing whatever was there. Returns how many
/// objects were registered and how many bytes they are.
pub fn register(budget: usize) -> Result<(u32, u64)> {
    let manager = manager();
    let bytes = arena();
    // User input: below one tile holds nothing, above the static buffer
    // does not exist.
    let limit = budget.clamp(TILE_BYTES as usize, bytes.len());
    *manager = Some(Manager::new(Arena::new(&mut bytes[..limit])));

    let held = manager.as_mut().ok_or(mlos_abi::Error::NoProvider)?;
    held.capacity = Capacity {
        bytes: limit as u64,
        ..SYNTHETIC
    };
    held.attach(&tiers::BACKING)?;
    held.attach(&tiers::RECOMPUTE)?;
    // A disk if the machine has one; the model is registered against
    // whichever provider is present.
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
pub fn with<T>(visit: impl FnOnce(&mut Manager<'static, CAPACITY>) -> T) -> Option<T> {
    manager().as_mut().map(visit)
}
