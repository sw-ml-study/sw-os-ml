//! Reading more than one sector.
//!
//! Separate because it is arithmetic over a device, not a device
//! operation: it turns a byte range into a sequence of sector reads and
//! has no business knowing how any single one of them works.

use mlos_abi::{Error, Result};

use crate::{Block, SECTOR};

impl Block {
    /// Reads `into.len()` bytes starting at byte `offset`.
    ///
    /// Sector-aligned offsets only, which every caller here has: objects
    /// are placed on sector boundaries precisely so this never has to
    /// read-modify-return a partial sector.
    ///
    /// # Safety
    ///
    /// As [`Self::read_sector`].
    pub unsafe fn read_at(&self, offset: u64, into: &mut [u8]) -> Result<u32> {
        if offset % SECTOR as u64 != 0 {
            return Err(Error::BadObject);
        }
        let mut sector = offset / SECTOR as u64;
        let mut done = 0;
        let mut scratch = [0u8; SECTOR];
        while done < into.len() {
            // SAFETY: forwarded from this function's contract.
            unsafe { self.read_sector(sector, &mut scratch) }?;
            let take = (into.len() - done).min(SECTOR);
            into[done..done + take].copy_from_slice(&scratch[..take]);
            done += take;
            sector += 1;
        }
        Ok(done as u32)
    }
}
