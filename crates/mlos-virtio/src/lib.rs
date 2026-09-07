//! virtio over MMIO.
//!
//! Enough to drive a console, which is what MLOS needs first: Apple's
//! Virtualization.framework offers no PL011, so without this MLOS boots
//! under it and says nothing (`docs/architecture.md` s.8.1).
//!
//! It is also the first piece of the provider machinery M2 needs -- a
//! block device and a filesystem are the same transport with a different
//! device id -- which is why the transport is separated from the console
//! that happens to use it.

#![no_std]

mod console;
mod queue;
mod regs;

pub use console::Console;
pub use regs::{MAGIC, VERSION};

/// The device id a console reports.
pub const CONSOLE_ID: u32 = 3;

/// `VIRTIO_F_VERSION_1`: the device speaks the non-legacy interface.
///
/// Bit 32, so it lives in the second feature word -- which is the whole
/// reason the feature registers are selected a word at a time.
const VERSION_1: u32 = 1 << 0;

/// A virtio device at an MMIO window.
#[derive(Clone, Copy)]
pub struct Device {
    base: usize,
}

impl Device {
    /// Identifies the device at `base`, if there is one.
    ///
    /// QEMU lays out 32 slots whether or not anything is plugged into
    /// them, and an empty slot reads a device id of zero rather than
    /// failing -- so probing is how you find what is actually there.
    ///
    /// # Safety
    ///
    /// `base` must be a mapped virtio-mmio window.
    #[must_use]
    pub unsafe fn probe(base: usize) -> Option<(Self, u32)> {
        // SAFETY: forwarded from this function's contract.
        unsafe {
            if regs::read(base, regs::reg::MAGIC) != MAGIC
                || regs::read(base, regs::reg::VERSION) != VERSION
            {
                return None;
            }
            match regs::read(base, regs::reg::DEVICE_ID) {
                0 => None,
                id => Some((Self { base }, id)),
            }
        }
    }

    /// Walks the handshake to `FEATURES_OK`, accepting only
    /// `VIRTIO_F_VERSION_1`.
    ///
    /// Accepting nothing else is deliberate: every feature accepted is a
    /// behaviour the driver then has to implement, and a console needs
    /// none of them.
    ///
    /// # Safety
    ///
    /// Call once per device, before configuring any queue.
    pub unsafe fn negotiate(&self) -> bool {
        use regs::{reg, status};
        // SAFETY: forwarded. The order is the specification's: announce
        // ourselves, read what is offered, answer, then check the device
        // still agrees.
        unsafe {
            regs::write(self.base, reg::STATUS, 0);
            regs::write(self.base, reg::STATUS, status::ACKNOWLEDGE);
            regs::write(self.base, reg::STATUS, status::ACKNOWLEDGE | status::DRIVER);

            regs::write(self.base, reg::DEVICE_FEATURES_SEL, 1);
            let offered = regs::read(self.base, reg::DEVICE_FEATURES);
            regs::write(self.base, reg::DRIVER_FEATURES_SEL, 1);
            regs::write(self.base, reg::DRIVER_FEATURES, offered & VERSION_1);
            regs::write(self.base, reg::DRIVER_FEATURES_SEL, 0);
            regs::write(self.base, reg::DRIVER_FEATURES, 0);

            let settled = status::ACKNOWLEDGE | status::DRIVER | status::FEATURES_OK;
            regs::write(self.base, reg::STATUS, settled);
            regs::read(self.base, reg::STATUS) & status::FEATURES_OK != 0
        }
    }

    /// Declares the device usable.
    ///
    /// # Safety
    ///
    /// Every queue the driver intends to use must already be configured.
    pub unsafe fn ready(&self) {
        use regs::{reg, status};
        let all = status::ACKNOWLEDGE | status::DRIVER | status::FEATURES_OK | status::DRIVER_OK;
        // SAFETY: forwarded.
        unsafe { regs::write(self.base, reg::STATUS, all) };
    }

    /// The MMIO window.
    #[must_use]
    pub const fn base(&self) -> usize {
        self.base
    }
}
