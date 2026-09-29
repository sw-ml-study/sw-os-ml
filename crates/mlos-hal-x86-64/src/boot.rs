//! The PVH entry, and what it leaves behind for Rust to ask about.
//!
//! Invariant: the entry arrives in 32-bit protected mode, paging off, with
//! the `hvm_start_info` pointer in `%ebx`. Design and history:
//! docs/notes/mlos-hal-x86-64.md.

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
