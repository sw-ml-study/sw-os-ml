//! The tier that is really a disk: weights read over virtio-blk.
//!
//! Invariant: an object's sector is a function of its id, with no index
//! on disk. Design: docs/notes/mlos-synth.md.

use mlos_abi::{Error, Result};
use mlos_objtab::{CostNs, ProviderId};
use mlos_provider::{Cost, Located, Provider};
use mlos_virtio_blk::{Block, SECTOR};

use crate::model;

/// The provider slot a disk occupies.
pub const DISK: ProviderId = ProviderId(4);

/// The sector the model's weights end at, and the trace begins: one
/// device holds both.
pub const TRACE_AT: u64 = (model::LAYERS as u64 * model::TILES as u64 * model::TILE_BYTES as u64)
    / mlos_virtio_blk::SECTOR as u64;

/// A block device holding the model.
pub struct Disk {
    /// The device its bytes come off.
    pub block: Block,
}

impl Disk {
    /// Which sector an object starts at, from its layer and tensor alone.
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

    /// NVMe-shaped numbers, identical to `tiers::BACKING` so policy
    /// comparisons are not confounded by which tier served the read.
    fn cost(&self, _object: Located) -> Cost {
        Cost {
            latency: CostNs(3_000_000),
            bytes_per_ms: 1_000_000,
        }
    }
}
