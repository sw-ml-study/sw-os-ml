//! The tier that is really a disk.
//!
//! Replaces the pattern-filling stub for weights when a block device is
//! present. The bytes are the same either way, which is the point: the
//! test is that they now cross a virtqueue to get here, so "three tiers"
//! stops being a claim about the table and becomes a fact about where the
//! data is.

use mlos_abi::{Error, Result};
use mlos_objtab::{CostNs, ProviderId};
use mlos_provider::{Cost, Located, Provider};
use mlos_virtio_blk::{Block, SECTOR};

use crate::model;

/// The provider slot a disk occupies.
pub const DISK: ProviderId = ProviderId(4);

/// A block device holding the model.
pub struct Disk {
    /// The device its bytes come off. Public because `Disk` adds a layout
    /// and a cost and nothing else -- a constructor would be a formality
    /// around a single field.
    pub block: Block,
}

impl Disk {
    /// Which sector an object starts at.
    ///
    /// Laid out by id rather than by a table on disk: layer and tensor
    /// give a position directly, so there is no index to read before the
    /// first read. A real model file would need one; a synthetic one
    /// should not pretend to.
    #[must_use]
    pub fn sector_of(id: mlos_abi::ObjectId) -> u64 {
        let fields = id.fields();
        let index = u64::from(fields.layer) * u64::from(model::TILES) + u64::from(fields.tensor);
        index * (model::TILE_BYTES as u64 / SECTOR as u64)
    }
}

impl Provider for Disk {
    fn id(&self) -> ProviderId {
        DISK
    }

    fn read(&self, object: Located, offset: u32, into: &mut [u8]) -> Result<u32> {
        let at = Self::sector_of(object.id) * SECTOR as u64 + u64::from(offset);
        let want = into.len().min(object.size as usize);
        // SAFETY: `block` came from `Block::new`, which brought the device
        // up, and this runs on the boot core one request at a time.
        unsafe { self.block.read_at(at, &mut into[..want]) }.map_err(|_| Error::NoProvider)
    }

    /// Real NVMe-ish numbers, unchanged from the stub they replace, so a
    /// comparison of policies is not confounded by the tier getting
    /// cheaper underneath it.
    fn cost(&self, _object: Located) -> Cost {
        Cost {
            latency: CostNs(3_000_000),
            bytes_per_ms: 1_000_000,
        }
    }
}
