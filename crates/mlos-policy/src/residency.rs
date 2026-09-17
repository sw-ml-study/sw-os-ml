//! What a policy is allowed to see.
//!
//! Its own module because it is the half of the interface the KERNEL has
//! to satisfy, over an open-addressed table, while the simulator
//! satisfies it over a `Vec`. Everything about the shape of this trait is
//! a constraint on the kernel rather than a convenience for the policy.

use mlos_abi::ObjectId;
use mlos_objtab::ObjectMeta;

/// The resident set, as a policy is allowed to see it.
///
/// Indexed rather than iterable so the kernel can satisfy it over an
/// open-addressed table without materialising anything. A policy that
/// wants the whole set walks `0..len()`, and pays for it.
pub trait Residency {
    /// How many objects are resident.
    fn len(&self) -> usize;

    /// The `index`th resident object, in a stable order.
    ///
    /// Stable, not sorted: the order must not change between two calls
    /// with nothing in between, or a policy would return different
    /// victims for the same state and the replay would stop being
    /// deterministic.
    fn at(&self, index: usize) -> Option<(ObjectId, ObjectMeta)>;

    /// Whether anything is resident at all.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
