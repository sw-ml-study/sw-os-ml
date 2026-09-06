//! Bring-up: find out what this machine is, say so, then map it.
//!
//! Its own module so the entry point stays a single decision -- run this,
//! and if it declines, stop.

use mlos_hal::BootInfo;
use mlos_hal::MemoryKind;
use mlos_hal_aarch64::Pl011;
use mlos_machine::{Machine, reserve};

use crate::{banner, trap};

/// Probes the device tree, opens the console it names, reports the
/// machine, then turns on translation.
///
/// `None` if the device tree cannot be read or names no console. That is
/// deliberate rather than a fallback: a machine we cannot read is one we
/// cannot run on, and limping along on a guessed console address would
/// hide the failure instead of showing it.
///
/// # Safety
///
/// Called once, on the boot core, with the MMU off. `dtb` must be what the
/// arm64 boot protocol left in `x0`.
pub unsafe fn bring_up(dtb: *const u8) -> Option<()> {
    // SAFETY: forwarded to `describe`, whose contract this is.
    let machine = unsafe { describe(dtb) }?;
    let uart = machine.uart_base?;
    let mut console = Pl011::at(uart);

    let info = BootInfo {
        regions: machine.regions.as_slice(),
        cpu_count: machine.cpu_count,
    };
    banner::report(&mut console, dtb as usize, &info, uart);

    // SAFETY: boot core, MMU off, once. The map covers the kernel image
    // and its stack, so the code that must survive the switch is mapped.
    unsafe { mlos_mmu_aarch64::enable_identity_map(info.regions) };
    banner::mmu(&mut console);

    // SAFETY: boot core, once, with the console known.
    unsafe { arm_traps(&mut console, uart) };
    Some(())
}

/// Reads the device tree, then carves out what the loader already put in
/// the memory it describes.
///
/// # Safety
///
/// `dtb` must be what the boot protocol left in `x0`.
unsafe fn describe(dtb: *const u8) -> Option<Machine> {
    // SAFETY: `probe` validates the header before trusting any field, so a
    // pointer to something else is rejected rather than followed.
    let mut machine = unsafe { Machine::probe(dtb) }?;
    // The tree cannot know we are here; the linker script does.
    let (base, len) = mlos_hal_aarch64::extent();
    machine.regions = reserve(&machine.regions, base, len, MemoryKind::Kernel);
    Some(machine)
}

/// Installs the vector table, then faults on purpose to prove it fires.
///
/// The deliberate fault is the only way to test a fault handler without
/// having a real bug, and a fault handler that has never fired is a fault
/// handler nobody knows is broken. It reads an address no block maps,
/// which produces a level-1 translation fault -- the syndrome the console
/// then prints.
///
/// This goes away in the next step, when there is real work to do after
/// boot. Until then, stopping in the handler is no worse than stopping in
/// the park loop, and considerably more informative.
///
/// # Safety
///
/// Call once, on the boot core, after the console is known. Does not
/// return: the reporter halts.
unsafe fn arm_traps(console: &mut Pl011, uart: usize) {
    trap::publish_console(uart);
    // SAFETY: `trap::report` allocates nothing, takes no locks, and reads
    // only a published address -- so it is safe from an exception context.
    unsafe { mlos_trap_aarch64::install(trap::report) };
    banner::selftest(console);
    // SAFETY: none, and that is the point. This read is meant to fault.
    unsafe { core::ptr::read_volatile(0x2_0000_0000 as *const u64) };
}
