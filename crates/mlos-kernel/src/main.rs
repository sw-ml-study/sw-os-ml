//! The MLOS microkernel.
//!
//! Milestone M1 is bring-up (`docs/plan.md`); nothing ML-shaped belongs
//! here until the kernel boots. The object table is M2, and resisting it
//! until then is the point.

#![no_std]
#![no_main]

#[cfg(target_arch = "aarch64")]
mod banner;

use core::panic::PanicInfo;

// Only the aarch64 entry formats anything; on x86-64 this would be an
// unused import, which `-D warnings` correctly rejects.
#[cfg(target_arch = "aarch64")]
use mlos_hal::BootInfo;
#[cfg(target_arch = "aarch64")]
use mlos_hal_aarch64::{Machine, Pl011};

/// Kernel entry, reached from the architecture's `_start`.
///
/// `dtb` is the device tree pointer the arm64 boot protocol leaves in the
/// first argument register (step 005 earned that; before the Image header
/// it arrived as zero).
///
/// The console address is *discovered*, not assumed. Step 004 hardcoded
/// the QEMU `virt` PL011 base as an explicit crutch; this deletes it. The
/// consequence is deliberate: a machine whose device tree we cannot read
/// is a machine we cannot run on, so a failed probe parks silently rather
/// than limping on a guessed address. Diagnosing that is what the gdb stub
/// is for, and is a reason QEMU is the development target
/// (`docs/architecture.md` s.8.1).
///
/// # Safety
///
/// Called exactly once, by `_start`, on the boot core, with a valid stack
/// and `.bss` already zeroed. Never called from Rust.
#[cfg(target_arch = "aarch64")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mlos_main(dtb: *const u8) -> ! {
    // SAFETY: `dtb` is whatever the boot protocol put in x0. `probe`
    // validates the header before trusting any field, so a bad pointer is
    // rejected rather than followed.
    let Some(machine) = (unsafe { Machine::probe(dtb) }) else {
        halt();
    };
    let Some(uart) = machine.uart_base else {
        halt();
    };

    let mut console = Pl011::at(uart);
    let info = BootInfo {
        regions: &machine.regions[..machine.region_count],
        cpu_count: machine.cpu_count,
    };
    banner::report(&mut console, dtb as usize, &info, uart);
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
