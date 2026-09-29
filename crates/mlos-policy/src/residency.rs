//! What a policy is allowed to see.
//!
//! Invariant: the shape of this trait is a constraint on the kernel, which
//! satisfies it over an open-addressed table without materialising
//! anything. Design: docs/notes/mlos-policy.md.

use mlos_abi::ObjectId;
use mlos_objtab::ObjectMeta;

/// The resident set, as a policy is allowed to see it. Indexed, not
/// iterable; a policy that wants the whole set walks `0..len()`.
pub trait Residency {
    /// How many objects are resident.
    fn len(&self) -> usize;

    /// The `index`th resident object, in an order that is stable across
    /// two calls with nothing in between; otherwise a replay is not
    /// deterministic.
    fn at(&self, index: usize) -> Option<(ObjectId, ObjectMeta)>;

    /// Whether anything is resident at all.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The current tick, on the same one-based counter `NextUse::At` and
    /// `used_tick` are measured on.
    fn now(&self) -> u32;
}
