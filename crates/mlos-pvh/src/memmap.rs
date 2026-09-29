//! The PVH memory map: e820 records, in the order the loader wrote them.
//!
//! Invariant: a record type this code does not know is `Reserved`, never
//! usable. Design: docs/notes/mlos-pvh.md.

use mlos_hal::{MemoryKind, MemoryRegion};

/// Bytes per record: base `u64`, length `u64`, type `u32`, reserved `u32`.
pub const ENTRY_LEN: usize = 24;

/// e820 type 1: RAM.
const RAM: u32 = 1;
/// e820 type 3: ACPI tables, reclaimable once read.
const ACPI: u32 = 3;

/// Hands every record in `memmap` to `push` as a [`MemoryRegion`]: RAM is
/// `Usable`, ACPI tables `Reclaimable`, everything else `Reserved`.
/// Zero-length records are dropped; a trailing partial record is ignored.
pub fn regions(memmap: &[u8], mut push: impl FnMut(MemoryRegion)) {
    for record in memmap.chunks_exact(ENTRY_LEN) {
        let word = |at: usize, len: usize| {
            record[at..at + len]
                .iter()
                .rev()
                .fold(0u64, |acc, &b| acc << 8 | u64::from(b))
        };
        let (base, len) = (word(0, 8), word(8, 8));
        let kind = match word(16, 4) as u32 {
            RAM => MemoryKind::Usable,
            ACPI => MemoryKind::Reclaimable,
            _ => MemoryKind::Reserved,
        };
        if len > 0 {
            push(MemoryRegion { base, len, kind });
        }
    }
}
