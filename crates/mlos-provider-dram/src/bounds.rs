//! Proving a read stays inside the window.
//!
//! Invariant: every arithmetic step is checked, because the inputs come
//! from table metadata and are not trusted. Design:
//! docs/notes/mlos-provider-dram.md.

use mlos_abi::{Error, Result};
use mlos_provider::Located;

/// Where an object's bytes start, if `[handle + offset, + want)` lies
/// wholly inside `[base, base + length)`; `BadObject` otherwise, including
/// on overflow.
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
