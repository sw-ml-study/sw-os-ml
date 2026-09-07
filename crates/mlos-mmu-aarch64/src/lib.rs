//! aarch64 boot translation tables.
//!
//! An identity map -- virtual address equals physical -- built from a
//! single level-1 table of 1 GiB blocks, then the MMU switched on. Identity
//! because the code doing the switching must keep running across it: with
//! any other mapping, the instruction after `isb` is somewhere else.
//!
//! This is deliberately *not* an implementation of [`mlos_hal::PageTable`].
//! That trait describes arbitrary byte-range mapping, which needs a
//! multi-level walker, table allocation and block splitting. Nothing needs
//! that yet -- the object manager is M2 (`docs/plan.md`) -- and writing a
//! general mapper before there is a caller to shape it means writing the
//! wrong one. What exists here is what boot needs and no more.
//!
//! Everything below 1 GiB is device memory on QEMU `virt` (flash, GIC,
//! PL011, virtio-mmio, PCIe ECAM); RAM starts at `0x4000_0000`. The map
//! reflects that: block 0 is device, and RAM blocks come from the device
//! tree rather than from an assumption about how much there is.

#![no_std]
// Empty on any other architecture, so the workspace-wide gate can sweep
// every crate without a hand-maintained exclude list. The crate says where
// it applies; a list in .cargo/config.toml would say it somewhere else and
// then drift, which is exactly what happened before this line existed.
#![cfg(target_arch = "aarch64")]

mod descriptor;
mod regs;

use core::cell::UnsafeCell;

use mlos_hal::{MemoryKind, MemoryRegion};

/// A level-1 table: 512 entries, one 4 KiB page, naturally aligned.
///
/// `TTBR0_EL1` requires the alignment, and `UnsafeCell` rather than
/// `static mut` because the 2024 edition rejects references to the latter
/// -- correctly, since nothing else stops two of them existing.
#[repr(C, align(4096))]
struct Level1(UnsafeCell<[u64; 512]>);

// SAFETY: written exactly once, by `enable_identity_map`, on the boot core
// before any other core leaves its park loop, and read-only to hardware
// afterwards.
unsafe impl Sync for Level1 {}

/// The one set of boot tables.
static TABLE: Level1 = Level1(UnsafeCell::new([0; 512]));

/// Builds the identity map and turns the MMU on.
///
/// Blocks not covered by `regions` are left invalid, so a stray access to
/// memory the device tree never claimed faults instead of quietly
/// succeeding against nothing.
///
/// # Safety
///
/// Call once, on the boot core, with the MMU off. `regions` must describe
/// the memory the kernel and its stack actually occupy; if it does not,
/// execution stops at the instruction after translation comes on.
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
