//! The boot report: banner first, then what the entry and the loader said.

use core::fmt::Write;

use mlos_hal_x86_64 as hal;
use mlos_uart16550::Uart16550;

use crate::machine::Machine;

/// Set on every report, so QEMU's own failure status is never mistaken
/// for one.
const REACHED: u8 = 0x40;
/// `EFER.LMA` read back as set.
const LONG_MODE: u8 = 0x01;
/// `%ebx` pointed at a valid `hvm_start_info` with a memory map.
const START_INFO: u8 = 0x02;
/// The identity map used 1 GiB pages rather than 2 MiB.
const GIGABYTE_PAGES: u8 = 0x04;
/// The guest stopped because of a trap, not Ctrl-D.
const TRAPPED: u8 = 0x08;

/// The trap reporter: prints why the CPU stopped, then ends the guest
/// with the boot bits plus [`TRAPPED`], so `mlos run` and the tests get
/// an answer instead of a machine that has silently stopped.
pub fn fault(trap: &mlos_trap_x86_64::Trap) -> ! {
    mlos_trap_x86_64::describe(trap, &mut Uart16550::at(mlos_uart16550::COM1));
    hal::qemu_exit(crate::CODE.load(core::sync::atomic::Ordering::Relaxed) | TRAPPED)
}

/// Prints the banner and what was found, and returns it as report bits.
///
/// The banner is the first line, with the architecture in it -- the same
/// shape as the aarch64 kernel's `MLOS aarch64`.
pub fn report(console: &mut Uart16550, machine: Option<&Machine>) -> u8 {
    let (long_mode, gigabyte) = (hal::long_mode(), hal::gigabyte_pages());
    let _ = writeln!(console, "\nMLOS x86-64");
    let _ = writeln!(console, "console     16550 at {:#x}", console.base());
    let _ = writeln!(
        console,
        "long mode   {}",
        if long_mode { "yes" } else { "NO" }
    );
    let pages = if gigabyte { "1 GiB" } else { "2 MiB" };
    let _ = writeln!(console, "paging      identity, {pages} pages");
    describe(console, machine);
    let bits = [
        (long_mode, LONG_MODE),
        (machine.is_some(), START_INFO),
        (gigabyte, GIGABYTE_PAGES),
    ];
    bits.iter()
        .filter(|(on, _)| *on)
        .fold(REACHED, |code, (_, bit)| code | bit)
}

/// The loader's side: where the memory map and virtio slots came from.
fn describe(out: &mut Uart16550, machine: Option<&Machine>) {
    let Some(machine) = machine else {
        let _ = writeln!(
            out,
            "start_info  INVALID: no memory map, not starting the shell"
        );
        return;
    };
    let (regions, dropped) = (machine.regions.as_slice().len(), machine.dropped);
    let _ = writeln!(
        out,
        "start_info  valid; memory map of {regions} regions from PVH"
    );
    if dropped > 0 {
        let _ = writeln!(
            out,
            "            {dropped} more did not fit and are NOT in the map"
        );
    }
    virtio(out, machine.virtio);
}

/// Where the virtio-mmio slots came from: `microvm`'s command line.
fn virtio(out: &mut Uart16550, slots: Option<(usize, usize, u32)>) {
    let _ = match slots {
        Some((base, _, n)) => writeln!(
            out,
            "virtio      {n} slot(s) from the command line, lowest {base:#x}"
        ),
        None => writeln!(out, "virtio      none on the command line"),
    };
}
