//! One block request, as the device expects to see it.
//!
//! Three descriptors for one operation, which is the shape virtio uses
//! everywhere and the reason the queue supports chaining: a header the
//! device reads, a buffer it writes, and a status byte it writes to say
//! how it went. Merging them would be smaller and wrong -- the device
//! must not be able to write into the header that told it what to do.

/// Request type: read from the device.
pub const IN: u32 = 0;

/// The header, at the front of every request.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Header {
    /// What to do. [`IN`] to read.
    pub kind: u32,
    /// Reserved; must be zero.
    pub reserved: u32,
    /// Which 512-byte sector to start at.
    ///
    /// Always 512, whatever the underlying device's block size. Virtio
    /// fixes it, so a 4 KiB-sector disk still counts in 512s here, and a
    /// driver that assumes otherwise reads the wrong place on real
    /// hardware while working perfectly against a file.
    pub sector: u64,
}

/// Bytes in a virtio block sector.
pub const SECTOR: usize = 512;

/// The device's answer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Status {
    /// It worked.
    Ok = 0,
    /// The device failed.
    Error = 1,
    /// The device did not understand the request.
    Unsupported = 2,
}

impl Status {
    /// Reads a status byte, treating anything undefined as an error.
    ///
    /// Undefined rather than panicking: a device that answers something
    /// the specification does not define has failed, whatever it meant.
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Ok,
            2 => Self::Unsupported,
            _ => Self::Error,
        }
    }
}
