//! The x86-64 half of `mlos-kernel`: what `boot.rs`, `banner.rs` and
//! `handlers.rs` are for aarch64, kept in its own crate (saga
//! `mlos-x86-64`, `lanes/x86/`) so the two architectures never share a
//! file while both are moving. `mlos-kernel` links it with one line.
//! What turns out to be common moves back into shared crates later.
//!
//! Boot: the machine is described from what the PVH loader handed over
//! ([`machine`]) and reported ([`banner`]); interrupts are armed
//! ([`handlers`]); then `mlsh` runs on COM1, woken by its receive
//! interrupt. `Ctrl-D` ends the guest through QEMU's `isa-debug-exit`
//! with the report bits, so a test has an answer that is not text; the
//! layout is duplicated in `mlos-cli/src/x86.rs`.

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
        // A machine we cannot read is one we cannot run on: say so, then
        // wait for Ctrl-D rather than limp on with a guessed memory map.
        None => loop {
            idle();
        },
    }
}

/// What the shell does between keystrokes: sleeps until an interrupt,
/// unless bytes arrived since the shell last looked, and ends the guest
/// once `Ctrl-D` has been seen.
///
/// `Ctrl-D` ends the guest on the call AFTER it arrives. The shell drains
/// the queue between calls, so everything typed before it runs first.
///
/// Without a LAPIC (never on `microvm`, but not assumed) interrupts are
/// not armed and COM1 is polled instead, as it was before interrupts.
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
