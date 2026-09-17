//! A manager of one's own.
//!
//! Every test here builds its own arena and object table rather than using
//! the kernel's statics, so the cases are independent and can run in
//! parallel. `Box::leak` is how a test gets the `&'static mut [u8]` an
//! `Arena` wants: the memory lives as long as the process, which is
//! exactly the lifetime the kernel's arena has anyway.

// Shared by every test binary in this directory, each of which uses a
// different part of it -- so what is unused HERE is used next door.
#![allow(dead_code)]

use mlos_objman::{Arena, Lease, Manager};
use mlos_objtab::SessionId;
use mlos_synth::{LAYERS, TILES, model, tiers};

/// Objects the table can hold.
pub const CAPACITY: usize = 512;

/// A manager with the model registered and `acquired` tiles faulted in.
///
/// `budget` is deliberately allowed to be smaller than the model. That is
/// the whole premise -- RAM is the scarce resource, the model does not fit,
/// and what a snapshot is FOR is showing how far it got.
pub fn with(budget: usize, acquired: u32) -> Manager<'static, CAPACITY> {
    let bytes: &'static mut [u8] = Box::leak(vec![0u8; budget].into_boxed_slice());
    let mut manager = Manager::new(Arena::new(bytes));
    manager.attach(&tiers::BACKING).expect("a provider slot");
    manager.attach(&tiers::RECOMPUTE).expect("a provider slot");

    for layer in 0..LAYERS {
        for tensor in 0..TILES {
            let meta = model::weights();
            manager
                .register(model::tile(layer, tensor), meta)
                .expect("room in the table");
        }
        manager
            .register(model::activation(layer), model::activations())
            .expect("room in the table");
    }

    for index in 0..acquired {
        let (layer, tensor) = (index / u32::from(TILES), index % u32::from(TILES));
        let id = model::tile(layer as u16, tensor as u16);
        if manager.acquire(id, Lease::Streaming, SessionId(1)).is_err() {
            break; // the arena filled, which is a legitimate outcome
        }
    }
    manager
}

/// Walks every tile in order, stopping where the arena runs out.
///
/// The same order `mlos-lab`'s sweep uses, because that is the access
/// pattern a dense transformer has and the one the whole design is about.
pub fn sweep(manager: &mut Manager<'static, CAPACITY>) {
    for layer in 0..LAYERS {
        for tensor in 0..TILES {
            let id = model::tile(layer, tensor);
            if manager.acquire(id, Lease::Streaming, SessionId(1)).is_err() {
                return;
            }
        }
    }
}

/// The document this manager produces.
pub fn document(manager: &Manager<'static, CAPACITY>) -> String {
    let mut text = String::new();
    mlos_snapshot::of(&mut text, manager, "testrev");
    text.replace("\r\n", "\n")
}

/// The event stream this manager has recorded.
pub fn events(manager: &Manager<'static, CAPACITY>) -> String {
    let mut text = String::new();
    mlos_events::write(&mut text, &manager.events);
    text.replace("\r\n", "\n")
}
