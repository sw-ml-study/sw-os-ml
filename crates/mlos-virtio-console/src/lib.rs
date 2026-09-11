//! A virtio console.
//!
//! Its own crate, like every other virtio device. `mlos-virtio` is the
//! transport and the virtqueue; what rides on them is a driver, and
//! keeping the two apart is what stops the transport crate growing a
//! module every time a device is added.
//!
//! Transmit only, for now. That is what closes requirement N2: MLOS needs
//! to be able to *say* something under Virtualization.framework before
//! being able to listen there is worth anything.

#![no_std]

use core::{cell::UnsafeCell, fmt};

use mlos_device::Console as ConsoleTrait;

use mlos_virtio::{
    Device,
    queue::{self, Available, Descriptor, SIZE, Used, UsedRing},
};

/// Queue 1 is the transmit queue of port 0. Queue 0 is its receive queue.
const TRANSMIT: u32 = 1;

/// The rings, and the bytes the device reads.
///
/// Static because there is no allocator, and because the device reads
/// them by physical address: they must not move. Identity-mapped, so the
/// address this code sees is the one the device is given.
#[repr(C, align(64))]
struct Rings {
    descriptors: UnsafeCell<[Descriptor; SIZE]>,
    available: UnsafeCell<Available>,
    used: UnsafeCell<UsedRing>,
    buffer: UnsafeCell<[u8; 256]>,
}

// SAFETY: written only by the boot core, one submission at a time, and
// each submission waits for the device before the next begins.
unsafe impl Sync for Rings {}

/// The one console's rings.
static RINGS: Rings = Rings {
    descriptors: UnsafeCell::new(
        [Descriptor {
            address: 0,
            length: 0,
            flags: 0,
            next: 0,
        }; SIZE],
    ),
    available: UnsafeCell::new(Available {
        flags: 0,
        index: 0,
        ring: [0; SIZE],
    }),
    used: UnsafeCell::new(UsedRing {
        flags: 0,
        index: 0,
        ring: [Used { id: 0, length: 0 }; SIZE],
    }),
    buffer: UnsafeCell::new([0; 256]),
};

/// A virtio console.
#[derive(Clone, Copy)]
pub struct Console {
    device: Device,
}

impl Console {
    /// Brings up the console at `device`.
    ///
    /// # Safety
    ///
    /// `device` must be a probed virtio console, and this must be called
    /// once, on the boot core, before anything writes to the console.
    pub unsafe fn new(device: Device) -> Option<Self> {
        // SAFETY: forwarded. `RINGS` is static and identity-mapped, so
        // its address is what the device needs.
        unsafe {
            if !device.negotiate() {
                return None;
            }
            let rings = (
                RINGS.descriptors.get() as u64,
                RINGS.available.get() as u64,
                RINGS.used.get() as u64,
            );
            if !queue::configure(device.base(), TRANSMIT, rings) {
                return None;
            }
            device.ready();
        }
        Some(Self { device })
    }
}

impl ConsoleTrait for Console {
    /// Sends bytes, a bufferful at a time.
    ///
    /// Synchronously: each chunk waits for the device before the next is
    /// staged, which is what makes one static buffer safe to reuse.
    fn write(&self, bytes: &[u8]) {
        for chunk in bytes.chunks(256) {
            // SAFETY: single-threaded, and the previous chunk has already
            // been returned by the device, so nothing else refers to these.
            unsafe {
                let buffer = &mut *RINGS.buffer.get();
                buffer[..chunk.len()].copy_from_slice(chunk);
                (*RINGS.descriptors.get())[0] = Descriptor {
                    address: buffer.as_ptr() as u64,
                    length: chunk.len() as u32,
                    flags: 0, // device-readable: it is our output
                    next: 0,
                };
                queue::submit(
                    self.device.base(),
                    TRANSMIT,
                    &mut *RINGS.available.get(),
                    &*RINGS.used.get(),
                );
            }
        }
    }
}

impl fmt::Write for Console {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for byte in text.as_bytes() {
            if *byte == b'\n' {
                self.write(b"\r"); // terminals want CRLF; the kernel should not care
            }
            self.write(core::slice::from_ref(byte));
        }
        Ok(())
    }
}
