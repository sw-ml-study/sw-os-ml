//! What this machine turned out to have.
//!
//! Probed lazily rather than at boot: nothing needs a disk until a model
//! is registered, and a machine without one should not pay for the search
//! or be told about it.

use core::sync::atomic::{AtomicU64, Ordering};

use mlos_synth::disk::Disk;
use mlos_virtio::{BLOCK_ID, Device};
use mlos_virtio_blk::Block;

use crate::state;

/// Where this machine's virtio-mmio slots are, if the kernel has said.
///
/// Set at boot rather than discovered here, because the device tree is
/// the kernel's to read and this crate has no business parsing one.
static SLOTS: AtomicU64 = AtomicU64::new(0);

/// Tells this crate where to look for devices.
pub fn set_slots(base: usize, size: usize, count: u32) {
    let packed = (base as u64) << 24 | (size as u64 & 0xffff) << 8 | u64::from(count & 0xff);
    SLOTS.store(packed, Ordering::Relaxed);
}

/// Whether the weights are being read from a real block device.
#[must_use]
pub fn on_disk() -> bool {
    disk().is_some()
}

/// The block device, found once and kept.
pub(crate) fn disk() -> Option<&'static Disk> {
    state::remember_disk(probe)
}

/// Looks for a block device among the machine's virtio slots.
fn probe() -> Option<Disk> {
    let packed = SLOTS.load(Ordering::Relaxed);
    if packed == 0 {
        return None;
    }
    let (base, size) = ((packed >> 24) as usize, ((packed >> 8) & 0xffff) as usize);
    for slot in 0..(packed & 0xff) as usize {
        // SAFETY: windows the device tree described, identity-mapped.
        let found = unsafe { Device::probe(base + slot * size) };
        if let Some((device, BLOCK_ID)) = found {
            // SAFETY: a probed block device, brought up once.
            return unsafe { Block::new(device) }.map(|block| Disk { block });
        }
    }
    None
}
