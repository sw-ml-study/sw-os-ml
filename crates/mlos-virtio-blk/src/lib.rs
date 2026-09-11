//! A virtio block device.
//!
//! Enough to read. Writing is not needed: MLOS treats a block store as
//! where weights *come from*, and weights are immutable -- the one
//! property that makes an object shareable across every session using the
//! model. A tier that can be written to is a tier that has to be
//! invalidated, and nothing here wants that yet.
//!
//! This is what turns "three tiers" from a claim into a fact: with it, a
//! cold object's bytes genuinely live somewhere other than memory and
//! have to cross a queue to be used.

#![no_std]

mod range;
mod request;

use core::cell::UnsafeCell;

use mlos_abi::{Error, Result};
use mlos_virtio::{
    Device,
    queue::{self, Available, Descriptor, NEXT, SIZE, Used, UsedRing, WRITE},
};

pub use request::{SECTOR, Status};

/// The request queue. A block device has one.
const REQUESTS: u32 = 0;

/// Rings, and the buffers a request is built in.
///
/// Static because the device reads them by physical address: they must
/// not move, and there is no allocator to promise that any other way.
#[repr(C, align(64))]
struct Rings {
    descriptors: UnsafeCell<[Descriptor; SIZE]>,
    available: UnsafeCell<Available>,
    used: UnsafeCell<UsedRing>,
    header: UnsafeCell<request::Header>,
    data: UnsafeCell<[u8; SECTOR]>,
    status: UnsafeCell<u8>,
}

// SAFETY: one request at a time, on the boot core, each waiting for the
// device before the next begins.
unsafe impl Sync for Rings {}

/// The one block device's rings.
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
    header: UnsafeCell::new(request::Header {
        kind: 0,
        reserved: 0,
        sector: 0,
    }),
    data: UnsafeCell::new([0; SECTOR]),
    status: UnsafeCell::new(0),
};

/// A virtio block device.
#[derive(Clone, Copy)]
pub struct Block {
    device: Device,
}

impl Block {
    /// Brings up the block device at `device`.
    ///
    /// # Safety
    ///
    /// `device` must be a probed virtio block device, and this must run
    /// once, on the boot core.
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
            if !queue::configure(device.base(), REQUESTS, rings) {
                return None;
            }
            device.ready();
        }
        Some(Self { device })
    }

    /// Reads one sector into `into`, which must be a whole sector.
    ///
    /// A sector at a time because the static buffer is one sector, and a
    /// bigger buffer would only move the limit rather than remove it. A
    /// caller wanting more asks more than once, which is what
    /// [`Self::read_at`] does.
    ///
    /// # Safety
    ///
    /// The device must have been brought up by [`Self::new`].
    pub unsafe fn read_sector(&self, sector: u64, into: &mut [u8]) -> Result<()> {
        if into.len() != SECTOR {
            return Err(Error::BadObject);
        }
        // SAFETY: single-threaded, and the previous request has already
        // been returned by the device, so nothing else refers to these.
        unsafe {
            // 0xff, not 0: zero is `Ok`, so a device that never wrote the
            // status byte would look like one that succeeded.
            *RINGS.header.get() = request::Header {
                kind: request::IN,
                reserved: 0,
                sector,
            };
            *RINGS.status.get() = 0xff;
            self.chain();
            queue::submit(
                self.device.base(),
                REQUESTS,
                &mut *RINGS.available.get(),
                &*RINGS.used.get(),
            );
            self.collect(into)
        }
    }

    /// Reads the answer back, if the device says it worked.
    ///
    /// # Safety
    ///
    /// A request must have completed.
    unsafe fn collect(&self, into: &mut [u8]) -> Result<()> {
        // SAFETY: forwarded; the device has returned the chain.
        unsafe {
            match Status::from_u8(*RINGS.status.get()) {
                Status::Ok => {
                    into.copy_from_slice(&*RINGS.data.get());
                    Ok(())
                }
                _ => Err(Error::NoProvider),
            }
        }
    }

    /// Builds the three-descriptor chain for a read.
    ///
    /// The flags are the contract. The header is device-readable, so it
    /// carries no `WRITE`; the data and status buffers are
    /// device-writable, and a device given a header marked writable is
    /// entitled to scribble on the request that told it what to do.
    ///
    /// # Safety
    ///
    /// No request may be in flight.
    unsafe fn chain(&self) {
        // SAFETY: forwarded; one request at a time.
        let descriptors = unsafe { &mut *RINGS.descriptors.get() };
        descriptors[0] = Descriptor {
            address: RINGS.header.get() as u64,
            length: size_of::<request::Header>() as u32,
            flags: NEXT,
            next: 1,
        };
        descriptors[1] = Descriptor {
            address: RINGS.data.get() as u64,
            length: SECTOR as u32,
            flags: NEXT | WRITE,
            next: 2,
        };
        descriptors[2] = Descriptor {
            address: RINGS.status.get() as u64,
            length: 1,
            flags: WRITE,
            next: 0,
        };
    }
}
