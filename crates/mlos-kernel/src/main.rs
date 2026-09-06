//! The MLOS microkernel.
//!
//! Milestone M1 is bring-up (`docs/plan.md`); nothing ML-shaped belongs
//! here until the kernel boots. The object table is M2, and resisting it
//! until then is the point.

#![no_std]
#![no_main]

use core::panic::PanicInfo;

// Only the aarch64 entry formats anything; on x86-64 this would be an
// unused import, which `-D warnings` correctly rejects.
#[cfg(target_arch = "aarch64")]
use core::fmt::Write;

/// Kernel entry, reached from the architecture's `_start`.
///
/// `dtb` is whatever the loader left in the first argument register. On
/// aarch64 that is the device tree pointer, which step 005 will parse; it
/// is printed here so a boot that gets this far proves the register
/// survived the entry path.
///
/// # Safety
///
/// Called exactly once, by `_start`, on the boot core, with a valid stack
/// and `.bss` already zeroed. Never called from Rust.
#[cfg(target_arch = "aarch64")]
#[unsafe(no_mangle)]
pub extern "C" fn mlos_main(dtb: usize) -> ! {
    let mut console = mlos_hal_aarch64::early_console();
    let _ = writeln!(
        console,
        "\nMLOS aarch64 -- entry reached, stack up, bss zeroed"
    );
    let _ = writeln!(console, "  dtb {dtb:#018x}");
    halt()
}

/// Placeholder entry for x86-64.
///
/// x86-64 is milestone M6 (`docs/plan.md`). The target is kept building so
/// it cannot silently rot, but there is no HAL behind it yet, so this
/// parks rather than pretending.
#[cfg(target_arch = "x86_64")]
#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.boot")]
pub extern "C" fn _start() -> ! {
    core::arch::naked_asm!("1:", "hlt", "jmp 1b");
}

/// Last resort. There is no scheduler to yield to, so the machine stops.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    halt()
}

/// Park the CPU. `spin_loop` emits the architecture's yield hint, so a
/// parked core stops burning power.
fn halt() -> ! {
    loop {
        core::hint::spin_loop();
    }
}
