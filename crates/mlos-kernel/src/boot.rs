//! Bring-up: find out what this machine is, say so, then map it.
//!
//! Its own module so the entry point stays a single decision -- run this,
//! and if it declines, stop.

use mlos_hal::BootInfo;
use mlos_hal_aarch64::{Machine, Pl011};

use crate::banner;

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
    // SAFETY: `probe` validates the header before trusting any field, so a
    // pointer to something else is rejected rather than followed.
    let machine = unsafe { Machine::probe(dtb) }?;
    let uart = machine.uart_base?;
    let mut console = Pl011::at(uart);

    let info = BootInfo {
        regions: &machine.regions[..machine.region_count],
        cpu_count: machine.cpu_count,
    };
    banner::report(&mut console, dtb as usize, &info, uart);

    // SAFETY: boot core, MMU off, once. `info.regions` is the device
    // tree's own memory map, so the kernel image and its stack lie inside
    // it -- which is what makes the identity map cover the code that has
    // to survive the switch.
    unsafe { mlos_mmu_aarch64::enable_identity_map(info.regions) };

    banner::mmu(&mut console);
    Some(())
}
