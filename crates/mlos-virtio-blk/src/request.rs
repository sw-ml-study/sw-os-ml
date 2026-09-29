//! One block request, as the device sees it: a header it reads, a buffer
//! it writes, a status byte it writes.
//!
//! Invariant: the header is never device-writable. Design:
//! docs/notes/mlos-virtio-blk.md.

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
    /// Which 512-byte sector to start at. Always 512, whatever the
    /// underlying device's block size.
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
    /// Reads a status byte; anything the specification does not define
    /// is [`Status::Error`].
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Ok,
            2 => Self::Unsupported,
            _ => Self::Error,
        }
    }
}
