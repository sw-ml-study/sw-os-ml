//! Bring-up: find out what this machine is, say so, then map it.
//!
//! Its own module so the entry point stays a single decision -- run this,
//! and if it declines, stop.

use mlos_device::{Irq, IrqController, Timer};
use mlos_gic_aarch64::Gic;
use mlos_hal::BootInfo;
use mlos_hal::MemoryKind;
use mlos_hal_aarch64::{GenericTimer, Pl011, TIMER_PPI};
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

    let info = BootInfo::new(machine.regions.as_slice(), machine.cpu_count);
    banner::report(&mut console, dtb as usize, &info, uart);

    // SAFETY: boot core, MMU off, once. The map covers the kernel image
    // and its stack, so the code that must survive the switch is mapped.
    unsafe { mlos_mmu_aarch64::enable_identity_map(info.regions) };
    banner::mmu(&mut console);

    // SAFETY: boot core, once, with the console known.
    unsafe { arm_interrupts(&mut console, uart, machine.gic?) };

    // Nothing else to do yet; the timer interrupt is the only thing that
    // happens from here. `wfi` rather than a spin so the host CPU is not
    // burned waiting for it.
    loop {
        // SAFETY: waits for an interrupt. No memory effects.
        unsafe { core::arch::asm!("wfi", options(nomem, nostack)) };
    }
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

/// Installs the vector table, brings up the interrupt controller, and
/// starts the timer.
///
/// Order is load-bearing throughout, and each step is quiet when wrong:
/// vectors before anything can trap, the controller before an interrupt
/// has anywhere to go, the timer before it is unmasked, and the unmask
/// last. Unmasking early with a half-built controller behind it fires
/// immediately and repeatedly, which is far harder to read than a machine
/// that simply never ticks.
///
/// # Safety
///
/// Call once, on the boot core, with interrupts masked and the console
/// already known.
unsafe fn arm_interrupts(console: &mut Pl011, uart: usize, gic: (u64, u64)) {
    // SAFETY: boot core, once. Neither handler allocates or takes a lock.
    unsafe { mlos_trap_aarch64::install(trap::report, trap::on_irq) };

    // SAFETY: the windows came from the device tree's `arm,gic-v3` node.
    let gic = unsafe { Gic::new(gic.0 as usize, gic.1 as usize) };
    gic.enable(Irq(TIMER_PPI));

    // Twice a second: slow enough to read on a console, fast enough that
    // a boot capture of a few seconds shows time actually passing.
    let interval = GenericTimer.frequency().0 / 2;
    banner::interrupts(console, GenericTimer.frequency().0, TIMER_PPI);

    // SAFETY: boot core, interrupts still masked, nothing has run yet.
    unsafe { trap::publish(uart, gic, interval) };
    GenericTimer.arm(interval);

    // SAFETY: vectors installed, controller up, timer armed.
    unsafe { mlos_trap_aarch64::unmask() };
}
