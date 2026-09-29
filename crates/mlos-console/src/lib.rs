//! Choosing a console: PL011 or virtio, decided once at boot from the
//! device tree and `/chosen/bootargs`.
//!
//! Invariant: `Terminal` is `Copy` with no lifetime, so it can sit in a
//! static for interrupt handlers. Design: docs/notes/mlos-console.md.

#![no_std]

mod input;
mod io;

use mlos_machine::Machine;
use mlos_pl011::Pl011;
use mlos_virtio::{CONSOLE_ID, Device};

/// A console, of whichever kind this machine has. `Copy`, so it can be
/// published into a static without a lifetime or an allocation.
#[derive(Clone, Copy)]
pub enum Terminal {
    /// An Arm PrimeCell UART.
    Pl011(Pl011),
    /// A virtio console.
    Virtio(mlos_virtio_console::Console),
}

impl Terminal {
    /// Opens whichever console this machine has: the PL011 unless
    /// `/chosen/bootargs` names an `hvc` console, and the PL011 again if
    /// no virtio console comes up. `None` if there is neither.
    ///
    /// # Safety
    ///
    /// Call once, on the boot core. The addresses come from the device
    /// tree and must be mapped.
    #[must_use]
    pub unsafe fn open(machine: &Machine) -> Option<Self> {
        let wants_virtio = machine
            .bootargs
            .is_some_and(|args| args.contains("console=hvc"));
        if !wants_virtio && let Some(base) = machine.uart_base {
            return Some(Self::Pl011(Pl011::at(base)));
        }
        // SAFETY: forwarded; the windows come from the device tree.
        unsafe { Self::virtio(machine) }
            .or_else(|| machine.uart_base.map(|base| Self::Pl011(Pl011::at(base))))
    }

    /// Finds a virtio console among the slots the device tree describes,
    /// probing each: an empty slot reads a device id of zero.
    ///
    /// # Safety
    ///
    /// As [`Self::open`].
    unsafe fn virtio(machine: &Machine) -> Option<Self> {
        let (base, size) = machine.virtio?;
        for slot in 0..machine.virtio_count as usize {
            // SAFETY: the windows are contiguous and uniform, per the
            // device tree that described them.
            let found = unsafe { Device::probe(base + slot * size) };
            if let Some((device, CONSOLE_ID)) = found {
                // SAFETY: a probed console, brought up once.
                if let Some(console) = unsafe { mlos_virtio_console::Console::new(device) } {
                    return Some(Self::Virtio(console));
                }
            }
        }
        None
    }
}
