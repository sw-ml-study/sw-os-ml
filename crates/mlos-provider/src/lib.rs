//! Where an ML object's bytes can come from.
//!
//! Invariant: providers are the only layer that knows about physical
//! media; resolution belongs to the object table, not to them. Design and
//! history: docs/notes/mlos-provider.md.

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
    /// arrived. Fewer than asked is normal, not an error.
    fn read(&self, object: Located, offset: u32, into: &mut [u8]) -> Result<u32>;

    /// What this provider would charge to produce the object. A property
    /// of the medium, not of the object.
    fn cost(&self, object: Located) -> Cost;

    /// Asks for an object to be made ready, without waiting. The default
    /// does nothing, which is honest for a medium that cannot prefetch.
    fn prefetch(&self, object: Located) -> Result<()> {
        let _ = object;
        Ok(())
    }
}
