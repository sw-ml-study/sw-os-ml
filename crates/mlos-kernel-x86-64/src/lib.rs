//! The x86-64 half of `mlos-kernel`: what `boot.rs`, `banner.rs` and
//! `handlers.rs` are for aarch64, kept in its own crate (saga
//! `mlos-x86-64`, `lanes/x86/`) so the two architectures never share a
//! file while both are moving. `mlos-kernel` links it with one line.
//! What turns out to be common moves back into shared crates later.
//!
//! Step `x86-bootinfo`: the machine is described from what the PVH loader
//! handed over ([`machine`]), reported ([`banner`]), and then `mlsh` runs
//! on COM1 -- polled, because the receive interrupt is step
//! `x86-interrupts`. `Ctrl-D` still ends the guest through QEMU's
//! `isa-debug-exit` with the step-1 bits, so a test has an answer that is
//! not text; the layout is duplicated in `mlos-cli/src/x86.rs`.

#![no_std]
// Empty anywhere but a bare x86-64 target, like `mlos-hal-x86-64`.
#![cfg(all(target_arch = "x86_64", target_os = "none"))]

mod banner;
mod machine;

use core::sync::atomic::{AtomicU8, Ordering};

use mlos_hal_x86_64 as hal;
use mlos_uart16550::{COM1, Uart16550};

/// `Ctrl-D`: ends the guest, from anywhere in the shell.
const END: u8 = 0x04;

/// The report bits, kept for [`idle`] to exit with.
static CODE: AtomicU8 = AtomicU8::new(0);

/// Kernel entry, reached from `mlos-hal-x86-64`'s `_start` in long mode.
///
/// # Safety
///
/// Called exactly once, by `_start`, on the boot CPU, with a valid stack,
/// `.bss` zeroed, and `start_info` as the PVH loader passed it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mlos_main(start_info: *const u8) -> ! {
    let mut console = Uart16550::at(COM1);
    console.init();
    // SAFETY: forwarded from `_start`, which is this function's contract.
    let found = unsafe { machine::discover(start_info) };
    CODE.store(
        banner::report(&mut console, found.as_ref()),
        Ordering::Relaxed,
    );
    match found {
        Some(machine) => machine.run_shell(&mut console, idle),
        // A machine we cannot read is one we cannot run on: say so, then
        // wait for Ctrl-D rather than limp on with a guessed memory map.
        None => loop {
            idle();
        },
    }
}

/// What the shell does between keystrokes: moves whatever COM1 received
/// into `mlsh`'s input queue, and ends the guest on `Ctrl-D`.
///
/// Polling stands in for the receive interrupt until step
/// `x86-interrupts`; the queue is the same one the aarch64 interrupt
/// handler fills, so the shell cannot tell the difference.
fn idle() {
    while let Some(byte) = Uart16550::at(COM1).read() {
        if byte == END {
            hal::qemu_exit(CODE.load(Ordering::Relaxed));
        }
        let _ = mlos_queue::push(byte);
    }
    core::hint::spin_loop();
}
