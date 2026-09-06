//! The MLOS microkernel.
//!
//! Currently the two things a bare-metal Rust binary cannot start without:
//! an entry symbol for the loader to jump to, and a panic handler, because
//! there is no runtime underneath to supply either.
//!
//! Milestone M1 fills this in (`docs/plan.md`). Nothing ML-shaped belongs
//! here until the kernel boots; the object table is M2.

#![no_std]
#![no_main]

use core::panic::PanicInfo;

/// Kernel entry point.
///
/// Placed in `.text.boot`, which the linker script pins to the image base,
/// because a bare `-kernel` boot jumps to the load address rather than
/// reading an entry symbol.
///
/// Naked, and it has to be. A normal `extern "C"` function opens with a
/// stack push, and the loader hands us an undefined `SP`: the first
/// instruction faults into the (still zero) vector table at
/// `VBAR_EL1 + 0x200` before anything of ours retires. That was observed
/// under both TCG and HVF, so the entry point may not touch the stack
/// until step 004 establishes one.
///
/// Entered with the MMU off. Until step 004 does real bring-up, the safest
/// thing a kernel can do is park the core.
#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.boot")]
pub extern "C" fn _start() -> ! {
    #[cfg(target_arch = "aarch64")]
    core::arch::naked_asm!("1:", "wfe", "b 1b");

    #[cfg(target_arch = "x86_64")]
    core::arch::naked_asm!("1:", "hlt", "jmp 1b");
}

/// Last resort. There is no console to report on and no scheduler to
/// yield to, so the machine stops here.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    halt()
}

/// Park the CPU. `spin_loop` emits the architecture's yield hint (`wfe` on
/// aarch64, `pause` on x86-64) so a parked core stops burning power.
fn halt() -> ! {
    loop {
        core::hint::spin_loop();
    }
}
