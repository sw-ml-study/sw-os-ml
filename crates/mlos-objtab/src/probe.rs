//! Where to look for an object, and in what order: open addressing with
//! linear probing.
//!
//! Invariant: the probe sequence for an id is a fixed function of the id
//! and the capacity, so a lookup and the insert that placed it agree.
//! Design: docs/notes/mlos-objtab.md.

use mlos_abi::ObjectId;

/// Scatters an id across the table. Mixes the high bits down, because a
/// structured [`ObjectId`]'s low bits are not random.
#[must_use]
pub const fn start(id: ObjectId, capacity: usize) -> usize {
    const GOLDEN: u64 = 0x9e37_79b9_7f4a_7c15;
    let mixed = id.0.wrapping_mul(GOLDEN);
    (mixed >> 32) as usize % capacity
}

/// The slots to try, in order, starting from `start`.
pub fn sequence(id: ObjectId, capacity: usize) -> impl Iterator<Item = usize> {
    let first = start(id, capacity);
    (0..capacity).map(move |step| (first + step) % capacity)
}
