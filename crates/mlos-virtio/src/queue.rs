//! A split virtqueue, synchronous: submit one chain, spin until the
//! device returns it.
//!
//! Invariant: the descriptor and the available ring must be visible to
//! the device before `QUEUE_NOTIFY` is written; `submit` fences for it.
//! Design and history: docs/notes/mlos-virtio.md.

use core::sync::atomic::{Ordering, fence};

use crate::regs::{self, reg};

/// Descriptors per queue. A power of two because the ring index wraps by
/// masking.
pub const SIZE: usize = 8;

/// A descriptor: where a buffer is and how the device may use it.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Descriptor {
    /// Physical address. Identity-mapped, so also the virtual one.
    pub address: u64,
    /// Length in bytes.
    pub length: u32,
    /// [`NEXT`] to continue a chain, [`WRITE`] if the device fills this
    /// buffer rather than reading it, or zero.
    pub flags: u16,
    /// Next descriptor in a chain; meaningful only with [`NEXT`] set.
    pub next: u16,
}

/// Descriptor flag: another descriptor follows in `next`.
pub const NEXT: u16 = 1;

/// Descriptor flag: the device writes this buffer rather than reading it.
pub const WRITE: u16 = 2;

/// The ring the driver fills with descriptor indices.
#[repr(C, align(2))]
pub struct Available {
    /// Flags. Zero: we want to be interrupted.
    pub flags: u16,
    /// How many entries the driver has ever made available.
    pub index: u16,
    /// Descriptor indices.
    pub ring: [u16; SIZE],
}

/// One entry the device returns.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Used {
    /// Which descriptor it finished with.
    pub id: u32,
    /// How many bytes it wrote, for device-writable buffers.
    pub length: u32,
}

/// The ring the device fills with finished descriptors.
#[repr(C, align(4))]
pub struct UsedRing {
    /// Flags.
    pub flags: u16,
    /// How many entries the device has ever returned.
    pub index: u16,
    /// The entries.
    pub ring: [Used; SIZE],
}

/// Tells the device where a queue's three rings are, and enables it.
/// `false` if the device cannot give a queue of [`SIZE`].
///
/// # Safety
///
/// `base` must be a virtio-mmio window whose device is past
/// `FEATURES_OK`, and the three addresses must stay valid and mapped for
/// as long as the queue is ready.
pub unsafe fn configure(base: usize, queue: u32, rings: (u64, u64, u64)) -> bool {
    // SAFETY: forwarded from this function's contract.
    unsafe {
        regs::write(base, reg::QUEUE_SEL, queue);
        if regs::read(base, reg::QUEUE_NUM_MAX) < SIZE as u32 {
            return false; // the device will not give us a queue this big
        }
        regs::write(base, reg::QUEUE_NUM, SIZE as u32);
        regs::write_addr(base, reg::QUEUE_DESC, rings.0);
        regs::write_addr(base, reg::QUEUE_DRIVER, rings.1);
        regs::write_addr(base, reg::QUEUE_DEVICE, rings.2);
        regs::write(base, reg::QUEUE_READY, 1);
    }
    true
}

/// Publishes the chain beginning at descriptor 0 and waits for it back,
/// returning how many bytes the device wrote. Always descriptor 0: one
/// operation is in flight at a time, and the device follows `next`.
///
/// # Safety
///
/// `base` must be a configured, ready queue on a `DRIVER_OK` device, and
/// `available`/`used` the rings it was configured with.
pub unsafe fn submit(base: usize, queue: u32, available: &mut Available, used: &UsedRing) -> u32 {
    // SAFETY: the device writes `used` by DMA, so every read of it must be
    // volatile -- the compiler has no reason to expect it to change.
    let before = unsafe { core::ptr::read_volatile(&raw const used.index) };

    let slot = available.index as usize % SIZE;
    available.ring[slot] = 0;
    // The device reads the descriptor and the ring from memory, so both
    // must be visible before the index advances and the index before the
    // notify; without the fences the device can be pointed at a descriptor
    // that has not been written yet, which fails only under load.
    fence(Ordering::SeqCst);
    available.index = available.index.wrapping_add(1);
    fence(Ordering::SeqCst);

    // SAFETY: forwarded from this function's contract.
    unsafe { regs::write(base, reg::QUEUE_NOTIFY, queue) };
    // SAFETY: as above.
    unsafe { wait(used, before) }
}

/// Spins until the device returns something, then reports how much it
/// moved.
///
/// # Safety
///
/// `used` must be the ring the device was configured with.
unsafe fn wait(used: &UsedRing, before: u16) -> u32 {
    // SAFETY: volatile because the device, not this code, advances these.
    unsafe {
        while core::ptr::read_volatile(&raw const used.index) == before {
            core::hint::spin_loop();
        }
        let slot = before as usize % SIZE;
        core::ptr::read_volatile(&raw const used.ring[slot].length)
    }
}
