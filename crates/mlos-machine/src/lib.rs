//! The machine, as the device tree describes it.
//!
//! Answers exactly the questions MLOS asks at boot -- where is memory, how
//! many CPUs, where is the console, and where is the blob itself -- and
//! stops.
//!
//! Architecture-neutral: nothing here is aarch64-specific, because a
//! device tree is not. It lives outside `mlos-hal-aarch64` for that reason
//! and because that crate had no module budget left, which is the
//! `sw-checklist` gate doing what AGENTS.md says it should -- forcing the
//! split at the point the concern actually separates.

#![no_std]

mod regions;
mod reserve;
mod scan;

use mlos_fdt::{Fdt, Header};
use mlos_hal::MemoryKind;

pub use regions::{MAX_REGIONS, Regions};
pub use reserve::reserve;

/// What the device tree said, plus where the tree itself sits.
#[derive(Clone, Copy)]
pub struct Machine {
    /// The physical memory map.
    pub regions: Regions,
    /// How many CPUs the tree describes.
    pub cpu_count: u32,
    /// Base address of the console, if the tree names one.
    pub uart_base: Option<usize>,
    /// The console's interrupt number, if the tree gives one.
    pub uart_irq: Option<u32>,
    /// The lowest `virtio_mmio@` window and its size, with how many
    /// identical slots follow it.
    pub virtio: Option<(usize, usize)>,
    /// How many `virtio_mmio@` slots the tree describes.
    pub virtio_count: u32,
    /// `/chosen/bootargs`, which says which console was asked for.
    pub bootargs: Option<&'static str>,
    /// GICv3 distributor and redistributor bases, if the tree has them.
    pub gic: Option<(u64, u64)>,
    /// Where the blob itself lives, so it can be reclaimed once read.
    pub blob: (u64, u64),
}

impl Machine {
    /// Marks `[base, base + len)` as holding the kernel image.
    ///
    /// The tree describes what the MACHINE has; it knows nothing about
    /// what a loader put into it. Combining those two facts is this
    /// type's job, not its caller's.
    /// Reads the device tree the loader left at `dtb`.
    ///
    /// The blob's own extent is recorded as it is parsed. Firmware
    /// structures are real memory -- several kilobytes here, and much more
    /// on a machine with ACPI -- and on a system whose whole point is
    /// accounting for resident bytes, leaving them permanently unavailable
    /// would be an odd place to start.
    ///
    /// # Safety
    ///
    /// `dtb` must be what the boot protocol supplied: a device tree blob
    /// whose declared length is readable. Anything else is rejected by the
    /// header check rather than believed.
    #[must_use]
    pub fn reserving(self, base: u64, len: u64) -> Self {
        Self {
            regions: reserve(&self.regions, base, len, MemoryKind::Kernel),
            ..self
        }
    }

    /// Reads the device tree the loader left at `dtb`.
    ///
    /// # Safety
    ///
    /// `dtb` must be what the boot protocol supplied.
    pub unsafe fn probe(dtb: *const u8) -> Option<Self> {
        // SAFETY: forwarded from this function's contract.
        let fdt = unsafe { Fdt::from_ptr(dtb) }?;
        // SAFETY: `from_ptr` already validated a header here.
        let head = unsafe { core::slice::from_raw_parts(dtb, 40) };
        let size = Header::parse(head)?.total_size as u64;

        let mut scan = scan::Scan::default();
        fdt.walk(|event| scan.event(event))?;
        Some(Self::from_scan(&scan, dtb as u64, size))
    }

    /// Turns a finished walk into a machine, reserving the blob itself.
    ///
    /// Firmware structures are real memory -- several kilobytes here, and
    /// much more on a machine with ACPI -- and on a system whose whole
    /// premise is accounting for resident bytes, writing them off
    /// permanently would be an odd place to start.
    fn from_scan(scan: &scan::Scan<'static>, base: u64, size: u64) -> Self {
        let machine = Self {
            regions: scan.regions,
            cpu_count: scan.cpu_count,
            uart_base: scan.uart_base,
            uart_irq: scan.uart_irq,
            virtio: scan.virtio,
            virtio_count: scan.virtio_count,
            bootargs: scan.bootargs,
            // Only a v3 layout is understood; a v2 reports a CPU
            // interface in that second range, a different device.
            gic: scan.gic_v3.then_some(scan.gic_reg).flatten(),
            blob: (base, size),
        };
        Self {
            regions: reserve(&machine.regions, base, size, MemoryKind::Reclaimable),
            ..machine
        }
    }
}
