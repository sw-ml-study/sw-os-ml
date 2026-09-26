//! The x86-64 half of `mlos-kernel`: what `boot.rs`, `banner.rs` and
//! `handlers.rs` are for aarch64, kept in its own crate (saga
//! `mlos-x86-64`, `lanes/x86/`) so the two architectures never share a
//! file while both are moving. `mlos-kernel` links it with one line.
//! What turns out to be common moves back into shared crates later.
//!
//! Step `x86-console`: COM1 is the console. The banner is the first line,
//! the entry's findings follow, and receive is proved by echoing what
//! arrives until `Ctrl-D`. Then the machine exits through QEMU's
//! `isa-debug-exit` with the same bits step `x86-entry` reported, so a
//! test still gets an answer that does not depend on reading text. The
//! bit layout is duplicated in `mlos-cli/src/x86.rs`.

#![no_std]
// Empty anywhere but a bare x86-64 target, like `mlos-hal-x86-64`.
#![cfg(all(target_arch = "x86_64", target_os = "none"))]

use core::fmt::Write;

use mlos_device::Console;
use mlos_hal_x86_64 as hal;
use mlos_uart16550::{COM1, Uart16550};

/// Set on every report, so QEMU's own failure status is never mistaken
/// for one.
const REACHED: u8 = 0x40;
/// `EFER.LMA` read back as set.
const LONG_MODE: u8 = 0x01;
/// `%ebx` pointed at a real `hvm_start_info`.
const START_INFO: u8 = 0x02;
/// The identity map used 1 GiB pages rather than 2 MiB.
const GIGABYTE_PAGES: u8 = 0x04;
/// `Ctrl-D`: ends the echo, and with it the machine.
const END: u8 = 0x04;

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
    let code = report(&mut console, unsafe { hal::start_info_valid(start_info) });
    echo(&console);
    hal::qemu_exit(code)
}

/// Prints the banner and what the entry found, and returns them as bits.
///
/// The banner is the first line, with the architecture in it -- the same
/// shape as the aarch64 kernel's `MLOS aarch64`.
fn report(console: &mut Uart16550, start_info: bool) -> u8 {
    let (long_mode, gigabyte) = (hal::long_mode(), hal::gigabyte_pages());
    let yes = |on: bool| if on { "yes" } else { "NO" };
    let _ = writeln!(console, "\nMLOS x86-64");
    let _ = writeln!(console, "console     16550 at {:#x}", console.base());
    let _ = writeln!(console, "long mode   {}", yes(long_mode));
    let _ = writeln!(
        console,
        "start_info  {}",
        if start_info { "valid" } else { "INVALID" }
    );
    let pages = if gigabyte { "1 GiB" } else { "2 MiB" };
    let _ = writeln!(console, "paging      identity, {pages} pages");
    let _ = writeln!(console, "echo; Ctrl-D ends");
    let bits = [
        (long_mode, LONG_MODE),
        (start_info, START_INFO),
        (gigabyte, GIGABYTE_PAGES),
    ];
    bits.iter()
        .filter(|(on, _)| *on)
        .fold(REACHED, |code, (_, bit)| code | bit)
}

/// Echoes every byte that arrives until `Ctrl-D`.
///
/// Polled: the receive interrupt needs the IOAPIC (step `x86-interrupts`).
/// `hlt` is not an option yet either, with nothing to wake it. A carriage
/// return comes back as a newline, which the console turns into CRLF.
fn echo(console: &Uart16550) {
    loop {
        match console.read() {
            Some(END) => return,
            Some(b'\r') => console.write(b"\n"),
            Some(byte) => console.write(&[byte]),
            None => core::hint::spin_loop(),
        }
    }
}
