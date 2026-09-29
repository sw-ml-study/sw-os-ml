//! x86-64 interrupt controllers and clocks: the LAPIC, the IOAPIC, the
//! silenced 8259 PIC, and the TSC.
//!
//! Invariant: the LAPIC base is read from `IA32_APIC_BASE`, the IOAPIC is
//! at [`IOAPIC_BASE`], and both lie in the uncached device window. Design
//! and history: docs/notes/mlos-apic-x86-64.md.

#![no_std]
// Empty anywhere but a bare x86-64 target, like `mlos-hal-x86-64`.
#![cfg(all(target_arch = "x86_64", target_os = "none"))]

pub mod clock;
mod lapic;
mod timer;

pub use lapic::{Lapic, SPURIOUS};

use mlos_hal_x86_64::port::outb;

/// Where the IOAPIC is: the PC convention, and QEMU `microvm`'s.
pub const IOAPIC_BASE: u64 = 0xfec0_0000;

/// Where the silenced 8259s are parked: vectors nothing else uses, so a
/// spurious PIC interrupt could never be mistaken for a real one.
const PIC_VECTORS: u8 = 0xe0;

/// Silences the 8259 PICs: remapped away from the exception vectors,
/// then every line masked. The IOAPIC delivers everything from here.
pub fn disable_pic() {
    let icw = [
        (0x20, 0x11),
        (0xa0, 0x11), // ICW1: init, ICW4 follows
        (0x21, PIC_VECTORS),
        (0xa1, PIC_VECTORS + 8), // ICW2: vector bases
        (0x21, 0x04),
        (0xa1, 0x02), // ICW3: cascade on IRQ 2
        (0x21, 0x01),
        (0xa1, 0x01), // ICW4: 8086 mode
        (0x21, 0xff),
        (0xa1, 0xff), // mask every line
    ];
    for (port, value) in icw {
        // SAFETY: the 8259 command and data ports, written in the
        // initialisation sequence the chip defines.
        unsafe { outb(port, value) };
    }
}

/// Routes ISA interrupt `irq` (IOAPIC pin `irq`, no override on
/// `microvm`) to `vector` on the CPU whose LAPIC id is `apic_id`: fixed
/// delivery, physical destination, edge-triggered, active high -- what an
/// ISA line is.
pub fn route(irq: u8, vector: u8, apic_id: u8) {
    let index = 0x10 + 2 * u32::from(irq);
    for (reg, value) in [
        (index, u32::from(vector)),
        (index + 1, u32::from(apic_id) << 24),
    ] {
        // SAFETY: IOREGSEL (+0x00) and IOWIN (+0x10) of the IOAPIC, in the
        // uncached device window; 32-bit accesses, as the IOAPIC requires.
        unsafe {
            ((IOAPIC_BASE) as *mut u32).write_volatile(reg);
            ((IOAPIC_BASE + 0x10) as *mut u32).write_volatile(value);
        }
    }
}
