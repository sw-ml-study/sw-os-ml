//! Where to look for an object, and in what order.
//!
//! Open addressing with linear probing. A tree would be the obvious
//! alternative and is the wrong one: the fast path is an exact-match
//! lookup on the model-fault path, and it must be a couple of cache lines
//! rather than a traversal. Linear probing keeps a miss in the same cache
//! line as the hit it displaced.

use mlos_abi::ObjectId;

/// Scatters an id across the table.
///
/// [`ObjectId`] is structured -- class, model, layer, tensor, tile -- so
/// its low bits are anything but random: a dense layer sweep walks
/// consecutive tensors, and masking the raw value would pile a whole
/// layer into adjacent slots. Fibonacci hashing mixes the high bits down,
/// which is what makes a sweep spread out instead of collide.
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
