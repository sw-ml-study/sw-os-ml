//! What the PVH loader hands an x86-64 guest, read from bytes.
//!
//! On x86-64 this is the device tree's counterpart (`docs/plan.md`, saga
//! `mlos-x86-64`): `hvm_start_info` carries the memory map, and the
//! command line carries both `mlsh.run=` and -- on QEMU `microvm` -- one
//! `virtio_mmio.device=` per transport. So `BootInfo` is filled from two
//! sources here where aarch64 fills it from one.
//!
//! Parsing only, from byte slices, with no `unsafe`: turning a physical
//! address into a slice is `mlos-hal-x86-64`'s job, and keeping it there
//! is what lets every rule in this crate be tested on the host.

#![no_std]
#![forbid(unsafe_code)]

mod cmdline;
mod memmap;

pub use cmdline::{user_args, virtio_slots};
pub use memmap::{ENTRY_LEN, regions};

/// The magic an `hvm_start_info` begins with.
pub const MAGIC: u32 = 0x336e_c578;

/// Bytes in an `hvm_start_info`, version 1 -- the version with a memory
/// map, and the one QEMU's PVH loader writes.
pub const HEADER_LEN: usize = 56;

/// The parts of `hvm_start_info` MLOS uses. Addresses are physical.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StartInfo {
    /// Structure version. The memory map exists from version 1.
    pub version: u32,
    /// The NUL-terminated command line, or 0.
    pub cmdline: u64,
    /// The ACPI RSDP, or 0. Unused until something needs ACPI.
    pub rsdp: u64,
    /// The memory map, `memmap_entries` records of [`ENTRY_LEN`] bytes.
    pub memmap: u64,
    /// How many records the memory map holds.
    pub memmap_entries: u32,
}

impl StartInfo {
    /// Reads a start info from its first [`HEADER_LEN`] bytes.
    ///
    /// `None` if the magic is wrong or the version predates the memory
    /// map: a machine without a map is one whose RAM we would be guessing
    /// at, and MLOS does not guess.
    #[must_use]
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let u32_at = |at: usize| Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?));
        let u64_at = |at: usize| Some(u64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?));
        if u32_at(0)? != MAGIC || u32_at(4)? < 1 {
            return None;
        }
        Some(Self {
            version: u32_at(4)?,
            cmdline: u64_at(24)?,
            rsdp: u64_at(32)?,
            memmap: u64_at(40)?,
            memmap_entries: u32_at(48)?,
        })
    }
}
