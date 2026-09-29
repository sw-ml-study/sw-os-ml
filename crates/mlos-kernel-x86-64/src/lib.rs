//! The x86-64 half of `mlos-kernel`: boot, the banner, and the interrupt
//! handlers, in a crate of its own so the two architectures never share a
//! file.
//!
//! Invariant: the guest ends through QEMU's `isa-debug-exit` with the
//! report bits, whose layout `mlos-cli/src/x86.rs` duplicates. Design and
//! history: docs/notes/mlos-kernel-x86-64.md.

#![no_std]
// Empty anywhere but a bare x86-64 target, like `mlos-hal-x86-64`.
#![cfg(all(target_arch = "x86_64", target_os = "none"))]

mod banner;
mod handlers;
mod machine;

use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use mlos_hal_x86_64 as hal;
use mlos_uart16550::{COM1, Uart16550};

/// `Ctrl-D`: ends the guest, from anywhere in the shell.
const END: u8 = 0x04;

/// The report bits, kept for [`idle`] and [`banner::fault`] to exit with.
static CODE: AtomicU8 = AtomicU8::new(0);

/// `Ctrl-D` has arrived; the guest ends once the shell has caught up.
static ENDING: AtomicBool = AtomicBool::new(false);

/// Interrupts are armed, so idle sleeps instead of polling.
static ARMED: AtomicBool = AtomicBool::new(false);

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
    // SAFETY: boot CPU, once, interrupts off; both handlers only touch
    // COM1, atomics and the LAPIC, and never allocate.
    unsafe { mlos_trap_x86_64::install(banner::fault, handlers::on_irq) };
    // SAFETY: forwarded from `_start`, which is this function's contract.
    let found = unsafe { machine::discover(start_info) };
    let code = banner::report(&mut console, found.as_ref());
    CODE.store(code, Ordering::Relaxed);
    let clock = handlers::arm(&mut console);
    ARMED.store(clock.is_some(), Ordering::Relaxed);
    let _ = core::fmt::Write::write_str(&mut console, "Ctrl-D ends the guest\n");
    match found {
        Some(machine) => machine.run_shell(&mut console, idle, clock.unwrap_or(0)),
        // Never run on a guessed memory map: wait for Ctrl-D instead.
        None => loop {
            idle();
        },
    }
}

/// What the shell does between keystrokes: sleeps until an interrupt
/// unless bytes arrived since the shell last looked, polls COM1 if
/// interrupts were never armed, and ends the guest on the call AFTER
/// `Ctrl-D` arrived, so everything typed before it has run.
fn idle() {
    if ENDING.load(Ordering::Relaxed) {
        hal::qemu_exit(CODE.load(Ordering::Relaxed));
    }
    if ARMED.load(Ordering::Relaxed) {
        hal::wait_unless(&handlers::PENDING);
    } else {
        handlers::receive();
        core::hint::spin_loop();
    }
}
