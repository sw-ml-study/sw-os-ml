//! Where an ML object can come from.
//!
//! Providers are the only place that knows about physical media. Above
//! them, an object has a class, a size and a next-use distance; below
//! them, it is bytes on some device. That separation is what lets the
//! kernel move an object between tiers while a consumer holds a handle to
//! it, which is the whole claim in `docs/architecture.md` s.1.
//!
//! **`resolve` is deliberately absent**, though `docs/design.md` s.6
//! lists it. Resolution is the object table's job: an id yields a
//! `ObjectMeta`, which names the provider and carries the handle. Asking
//! the provider to resolve as well would be a second lookup answering a
//! question already answered, on the fault path, where there is least
//! room for one.

#![no_std]
#![forbid(unsafe_code)]

mod located;

use mlos_abi::Result;
use mlos_objtab::ProviderId;

pub use located::{Cost, Located};

/// Something that can produce an object's bytes.
pub trait Provider {
    /// Which provider this is, as the object table refers to it.
    fn id(&self) -> ProviderId;

    /// Copies part of an object into `into`, returning how many bytes
    /// arrived.
    ///
    /// Partial reads are normal, not an error: an object may be larger
    /// than the buffer a caller is willing to give, and a weight tile is
    /// consumed in pieces.
    fn read(&self, object: Located, offset: u32, into: &mut [u8]) -> Result<u32>;

    /// What this provider would charge to produce the object.
    ///
    /// A property of the medium, not of the object, so the default is a
    /// constant a provider overrides once rather than computing per call.
    fn cost(&self, object: Located) -> Cost;

    /// Asks for an object to be made ready, without waiting.
    ///
    /// The default does nothing, which is honest for a medium with no
    /// notion of getting ready: resident memory cannot be prefetched, and
    /// pretending otherwise would have policies counting hits that never
    /// happened.
    fn prefetch(&self, object: Located) -> Result<()> {
        let _ = object;
        Ok(())
    }
}
