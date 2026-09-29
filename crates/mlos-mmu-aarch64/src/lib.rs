//! aarch64 boot translation tables: an identity map of 1 GiB blocks.
//!
//! Invariant: the map covers the code, stack and console in use when the
//! MMU comes on, or execution stops at the next instruction. Design and
//! history: docs/notes/mlos-mmu-aarch64.md.

#![no_std]
// Empty on any other architecture, so the workspace gate can sweep every
// crate without an exclude list.
#![cfg(target_arch = "aarch64")]

mod descriptor;
mod regs;

use core::cell::UnsafeCell;

use mlos_hal::{MemoryKind, MemoryRegion};

/// A level-1 table: 512 entries, one 4 KiB page, aligned as `TTBR0_EL1`
/// requires.
#[repr(C, align(4096))]
struct Level1(UnsafeCell<[u64; 512]>);

// SAFETY: written exactly once, by `enable_identity_map`, on the boot core
// before any other core leaves its park loop, and read-only to hardware
// afterwards.
unsafe impl Sync for Level1 {}

/// The one set of boot tables.
static TABLE: Level1 = Level1(UnsafeCell::new([0; 512]));

/// Builds the identity map and turns the MMU on. Blocks not covered by
/// `regions` are left invalid, so a stray access faults.
///
/// # Safety
///
/// Call once, on the boot core, with the MMU off. `regions` must describe
/// the memory the kernel and its stack actually occupy.
pub unsafe fn enable_identity_map(regions: &[MemoryRegion]) {
    let table = TABLE.0.get();
    // SAFETY: called once, on the boot core, before any other core leaves
    // its park loop -- so this is the only reference to TABLE in
    // existence, and nothing is walking it yet.
    let entries = unsafe { &mut *table };

    for (index, entry) in (0u64..).zip(entries.iter_mut()) {
        let base = index * descriptor::BLOCK_SIZE;
        *entry = if base < descriptor::BLOCK_SIZE {
            descriptor::device(base) // everything below 1 GiB on `virt`
        } else if covered(regions, base) {
            descriptor::normal(base)
        } else {
            0 // invalid: fault rather than pretend
        };
    }

    // SAFETY: the table is complete, and maps the kernel image, its stack
    // and the console -- which is what surviving `turn_on` requires.
    unsafe {
        regs::install(table as u64);
        regs::turn_on();
    }
}

/// Whether translation is currently enabled.
#[must_use]
pub fn is_enabled() -> bool {
    regs::is_enabled()
}

/// Whether any usable region overlaps the 1 GiB block starting at `base`.
fn covered(regions: &[MemoryRegion], base: u64) -> bool {
    let end = base + descriptor::BLOCK_SIZE;
    regions
        .iter()
        .filter(|region| region.kind != MemoryKind::Device)
        .any(|region| region.base < end && base < region.base + region.len)
}
