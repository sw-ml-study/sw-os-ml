//! The PVH entry, and what it leaves behind for Rust to ask about.
//!
//! PVH is the x86-64 counterpart of `-kernel Image` on arm64: QEMU finds
//! a `XEN_ELFNOTE_PHYS32_ENTRY` note in the ELF, loads the segments at
//! their physical addresses, and jumps to the note's address in 32-bit
//! protected mode with paging off and a `hvm_start_info` in `%ebx`. No
//! firmware and no image conversion -- the same trade M1 made.

use core::arch::{asm, global_asm};

global_asm!(include_str!("entry.s"));

unsafe extern "C" {
    /// Non-zero once `entry.s` has mapped with 1 GiB pages. Written once,
    /// before any Rust runs.
    static mlos_gigabyte_pages: u8;
}

/// The magic a real `hvm_start_info` starts with (`"xEn3"` with bit 7 of
/// the last byte set, as the PVH ABI spells it).
const START_INFO_MAGIC: u32 = 0x336e_c578;

/// True if the boot map used 1 GiB pages, false if it fell back to 2 MiB.
///
/// Reported rather than hidden: page size changes TLB reach, and someone
/// comparing `sweep` timings across CPUs will eventually need to know.
/// QEMU's default CPU lacks `pdpe1gb`; `-cpu max` has it.
#[must_use]
pub fn gigabyte_pages() -> bool {
    // SAFETY: a byte in `.bss`, written only by `entry.s` before `mlos_main`.
    unsafe { (&raw const mlos_gigabyte_pages).read_volatile() != 0 }
}

/// True if the CPU says it is in long mode (`EFER.LMA`), read back from
/// the hardware rather than assumed from having got this far.
#[must_use]
pub fn long_mode() -> bool {
    let low: u32;
    // SAFETY: EFER (MSR 0xC000_0080) exists on every x86-64 CPU and
    // reading it has no side effects.
    unsafe {
        asm!("rdmsr", in("ecx") 0xC000_0080u32, out("eax") low, out("edx") _,
             options(nomem, nostack));
    }
    low & (1 << 10) != 0
}

/// True if `start_info` points at a PVH `hvm_start_info`.
///
/// # Safety
///
/// `start_info` must be the pointer the PVH entry received in `%ebx`,
/// which the 1 GiB identity map covers, or null.
#[must_use]
pub unsafe fn start_info_valid(start_info: *const u8) -> bool {
    if start_info.is_null() {
        return false;
    }
    // SAFETY: per the contract, a mapped PVH structure whose first field
    // is a u32; unaligned read because the ABI does not promise more.
    unsafe { start_info.cast::<u32>().read_unaligned() == START_INFO_MAGIC }
}
