//! Proving a read stays inside the window.
//!
//! Its own module because it is the only thing here that can be wrong in
//! a way that matters. The inputs come from a table an object's own
//! metadata populated, and treating that as trusted is how a wrong `size`
//! becomes a read of somebody else's memory.

use mlos_abi::{Error, Result};
use mlos_provider::Located;

/// Where an object's bytes start, if the request is inside the window.
///
/// Every arithmetic step is checked. The inputs come from a table an
/// object's own metadata populated, and treating that as trusted is
/// how a wrong `size` becomes a read of somebody else's memory.
pub fn within(base: u64, length: u64, object: Located, offset: u32, want: usize) -> Result<u64> {
    let start = object
        .handle
        .checked_add(u64::from(offset))
        .ok_or(Error::BadObject)?;
    let end = start.checked_add(want as u64).ok_or(Error::BadObject)?;
    let limit = base.checked_add(length).ok_or(Error::BadObject)?;
    if start < base || end > limit {
        return Err(Error::BadObject);
    }
    Ok(start)
}
