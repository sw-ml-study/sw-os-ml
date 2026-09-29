//! virtio over MMIO: the transport and the virtqueue every virtio device
//! driver rides on.
//!
//! Invariant: only `VIRTIO_F_VERSION_1` is ever accepted, so no driver
//! has to implement a feature it did not ask for. Design and history:
//! docs/notes/mlos-virtio.md.

#![no_std]

pub mod queue;
mod regs;

pub use regs::{MAGIC, VERSION};

/// The device id a console reports.
pub const CONSOLE_ID: u32 = 3;
/// The device id a block device reports.
pub const BLOCK_ID: u32 = 2;

/// `VIRTIO_F_VERSION_1`: the device speaks the non-legacy interface. Bit
/// 32, so it is word 1 of the feature registers.
const VERSION_1: u32 = 1 << 0;

/// A virtio device at an MMIO window.
#[derive(Clone, Copy)]
pub struct Device {
    base: usize,
}

impl Device {
    /// Identifies the device at `base`, if there is one. An empty slot
    /// reads a device id of zero and is reported as `None`.
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
    /// `VIRTIO_F_VERSION_1`. `false` if the device refused.
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
