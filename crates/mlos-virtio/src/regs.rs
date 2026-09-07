//! The virtio-mmio register block.
//!
//! Version 2 ("modern") only. Version 1 is a different memory layout with
//! a different queue-address convention, and supporting both would double
//! this file to serve hardware that no longer ships.

use core::ptr;

/// `0x74726976` -- "virt" little-endian. Anything else is not a device.
pub const MAGIC: u32 = 0x7472_6976;
/// The only version this driver speaks.
pub const VERSION: u32 = 2;

/// Register offsets.
///
/// Only the ones this driver uses. The interrupt registers are absent
/// because transmit spins for completion rather than waiting for one --
/// they arrive with receive, which needs a handler anyway.
pub mod reg {
    /// Magic value, must read as [`super::MAGIC`].
    pub const MAGIC: usize = 0x000;
    /// Transport version.
    pub const VERSION: usize = 0x004;
    /// What kind of device this is; 0 means the slot is empty.
    pub const DEVICE_ID: usize = 0x008;
    /// Features the device offers, 32 bits at a time.
    pub const DEVICE_FEATURES: usize = 0x010;
    /// Which 32-bit word of the device's features to read.
    pub const DEVICE_FEATURES_SEL: usize = 0x014;
    /// Features the driver accepts.
    pub const DRIVER_FEATURES: usize = 0x020;
    /// Which word of the driver's features to write.
    pub const DRIVER_FEATURES_SEL: usize = 0x024;
    /// Which queue the queue registers refer to.
    pub const QUEUE_SEL: usize = 0x030;
    /// Largest queue the device supports; 0 means the queue does not exist.
    pub const QUEUE_NUM_MAX: usize = 0x034;
    /// Queue size the driver chose.
    pub const QUEUE_NUM: usize = 0x038;
    /// Writing 1 tells the device the queue is usable.
    pub const QUEUE_READY: usize = 0x044;
    /// Writing a queue index tells the device to look at it.
    pub const QUEUE_NOTIFY: usize = 0x050;
    /// Driver status; the handshake lives here.
    pub const STATUS: usize = 0x070;
    /// Descriptor table address, low then high.
    pub const QUEUE_DESC: usize = 0x080;
    /// Available ring address.
    pub const QUEUE_DRIVER: usize = 0x090;
    /// Used ring address.
    pub const QUEUE_DEVICE: usize = 0x0a0;
}

/// Driver status bits, written in this order during bring-up.
pub mod status {
    /// The driver has noticed the device.
    pub const ACKNOWLEDGE: u32 = 1;
    /// The driver knows how to drive it.
    pub const DRIVER: u32 = 2;
    /// Setup is complete and the device may be used.
    pub const DRIVER_OK: u32 = 4;
    /// Feature negotiation is settled. The device may refuse.
    pub const FEATURES_OK: u32 = 8;
}

/// Reads a register.
///
/// # Safety
///
/// `base` must be a virtio-mmio window and `offset` within it.
#[must_use]
pub unsafe fn read(base: usize, offset: usize) -> u32 {
    // SAFETY: forwarded. Volatile because a device register's value is
    // not a function of what we last wrote.
    unsafe { ptr::read_volatile((base + offset) as *const u32) }
}

/// Writes a register.
///
/// # Safety
///
/// `base` must be a virtio-mmio window and `offset` within it.
pub unsafe fn write(base: usize, offset: usize, value: u32) {
    // SAFETY: forwarded.
    unsafe { ptr::write_volatile((base + offset) as *mut u32, value) };
}

/// Writes a 64-bit address as the low/high pair the transport expects.
///
/// # Safety
///
/// As [`write`].
pub unsafe fn write_addr(base: usize, offset: usize, address: u64) {
    // SAFETY: forwarded. The high half is always at offset + 4.
    unsafe {
        write(base, offset, address as u32);
        write(base, offset + 4, (address >> 32) as u32);
    }
}
