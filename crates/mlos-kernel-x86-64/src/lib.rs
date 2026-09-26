//! The x86-64 half of `mlos-kernel`: what `boot.rs`, `banner.rs` and
//! `handlers.rs` are for aarch64, kept in its own crate (saga
//! `mlos-x86-64`, `lanes/x86/`) so the two architectures never share a
//! file while both are moving. `mlos-kernel` links it with one line.
//! What turns out to be common moves back into shared crates later.
//!
//! Step `x86-entry`: the entry reaches [`mlos_main`] and nothing more.
//! There is no console yet -- that is step `x86-console` -- so the first
//! proof that the entry works is carried out through QEMU's exit status:
//! each bit of the code is a fact the kernel checked, and `mlos run
//! --arch x86-64` decodes them. The bit layout is duplicated in
//! `mlos-cli/src/x86.rs`; the boot test fails if the two disagree.

#![no_std]
// Empty anywhere but a bare x86-64 target, like `mlos-hal-x86-64`.
#![cfg(all(target_arch = "x86_64", target_os = "none"))]

use mlos_hal_x86_64 as hal;

/// Set on every report, so a status of 1 (QEMU's own failure) can never
/// be mistaken for one.
const REACHED: u8 = 0x40;
/// `EFER.LMA` read back as set.
const LONG_MODE: u8 = 0x01;
/// `%ebx` pointed at a real `hvm_start_info`.
const START_INFO: u8 = 0x02;
/// The identity map used 1 GiB pages rather than 2 MiB.
const GIGABYTE_PAGES: u8 = 0x04;

/// Kernel entry, reached from `mlos-hal-x86-64`'s `_start` in long mode.
///
/// # Safety
///
/// Called exactly once, by `_start`, on the boot CPU, with a valid stack,
/// `.bss` zeroed, and `start_info` as the PVH loader passed it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mlos_main(start_info: *const u8) -> ! {
    let mut code = REACHED;
    if hal::long_mode() {
        code |= LONG_MODE;
    }
    // SAFETY: forwarded from `_start`, which is this function's contract.
    if unsafe { hal::start_info_valid(start_info) } {
        code |= START_INFO;
    }
    if hal::gigabyte_pages() {
        code |= GIGABYTE_PAGES;
    }
    hal::qemu_exit(code)
}
