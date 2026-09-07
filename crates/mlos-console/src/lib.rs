//! Choosing a console.
//!
//! MLOS can talk to two kinds, and which one exists is a property of the
//! hypervisor rather than of the architecture: QEMU's `virt` has a PL011,
//! Apple's Virtualization.framework has only a virtio console. A kernel
//! that drives one of them boots under one of them and says nothing under
//! the other.
//!
//! The choice is made once, at boot, from what the device tree describes
//! and what `/chosen/bootargs` asks for -- the same `console=` convention
//! Linux uses, so a host that already knows how to ask gets what it asked
//! for.

#![no_std]

mod input;
mod io;

use mlos_machine::Machine;
use mlos_pl011::Pl011;
use mlos_virtio::{CONSOLE_ID, Device};

/// A console, of whichever kind this machine has.
///
/// An enum rather than `dyn Console`: it is `Copy`, so it can be published
/// into a static for interrupt handlers to find without a lifetime or an
/// allocation, and the fault reporter can hold one on an arbitrary stack.
#[derive(Clone, Copy)]
pub enum Terminal {
    /// An Arm PrimeCell UART.
    Pl011(Pl011),
    /// A virtio console.
    Virtio(mlos_virtio::Console),
}

impl Terminal {
    /// Opens whichever console this machine has.
    ///
    /// Prefers the PL011 unless `/chosen/bootargs` names an `hvc` console,
    /// because the PL011 needs no setup and works before anything else
    /// does -- which is what you want when the thing you are debugging is
    /// the console. `console=hvc0` overrides that, and is also how the
    /// virtio path gets exercised on a machine that has both.
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

    /// Finds a virtio console among the slots the device tree describes.
    ///
    /// Probing, not indexing: QEMU lays out 32 identical windows whether
    /// or not anything is plugged into them, and an empty one reads a
    /// device id of zero rather than failing.
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
                if let Some(console) = unsafe { mlos_virtio::Console::new(device) } {
                    return Some(Self::Virtio(console));
                }
            }
        }
        None
    }
}
