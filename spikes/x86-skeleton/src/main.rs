//! Spike: the smallest x86-64 MLOS-shaped kernel that proves the toolchain.
//!
//! Boots via PVH (`src/boot.s`), reaches long mode, reports on COM1, and
//! exits QEMU through `isa-debug-exit` so a host test gets a real status.

#![no_std]
#![no_main]

mod port;

use core::fmt::Write;
use core::panic::PanicInfo;
use port::{Serial, cpuid_vendor, efer_lma, exit_qemu};

core::arch::global_asm!(include_str!("boot.s"));

/// Status the host test expects: QEMU exits with `(code << 1) | 1` = 33.
const SUCCESS: u32 = 0x10;

/// Entered from `long_mode` in `boot.s` with the PVH start-info pointer.
#[unsafe(no_mangle)]
extern "C" fn kmain(start_info: u32) -> ! {
    let mut com1 = Serial::com1();
    let _ = writeln!(com1, "\nMLOS x86_64");
    let _ = writeln!(com1, "long mode: {}", if efer_lma() { "yes" } else { "no" });
    let _ = writeln!(com1, "pvh start_info: {start_info:#x}");
    let _ = writeln!(com1, "hypervisor: {}", cpuid_vendor());
    exit_qemu(SUCCESS)
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let _ = writeln!(Serial::com1(), "panic: {info}");
    exit_qemu(0x11)
}
