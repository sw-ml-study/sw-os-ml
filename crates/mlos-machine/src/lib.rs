//! The machine, as the device tree describes it: memory, CPUs, console,
//! interrupt controller, virtio, and where the blob itself sits.
//!
//! Invariant: architecture-neutral; nothing here is aarch64-specific.
//! Design and history: docs/notes/mlos-machine.md.

#![no_std]

mod regions;
mod reserve;
mod scan;

use mlos_fdt::{Fdt, Header};
use mlos_hal::MemoryKind;

pub use regions::{MAX_REGIONS, Regions};

/// Everything after `key` in a boot-args string, to the end of it, not
/// to the next space; the caller narrows a single-token setting. A
/// setting whose value contains spaces must therefore come last.
#[must_use]
pub fn rest<'a>(bootargs: &'a str, key: &str) -> &'a str {
    match bootargs.split_once(key) {
        Some((_, rest)) => rest,
        None => "",
    }
}
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
    /// Marks `[base, base + len)` as holding the kernel image, which the
    /// tree cannot know about.
    #[must_use]
    pub fn reserving(self, base: u64, len: u64) -> Self {
        Self {
            regions: reserve(&self.regions, base, len, MemoryKind::Kernel),
            ..self
        }
    }

    /// Reads the device tree the loader left at `dtb`, recording the
    /// blob's own extent as reclaimable.
    ///
    /// # Safety
    ///
    /// `dtb` must be what the boot protocol supplied: a device tree blob
    /// whose declared length is readable. Anything else is rejected by the
    /// header check rather than believed.
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
    fn from_scan(scan: &scan::Scan<'static>, base: u64, size: u64) -> Self {
        let machine = Self {
            regions: scan.regions,
            cpu_count: scan.cpu_count,
            uart_base: scan.uart_base,
            uart_irq: scan.uart_irq,
            virtio: scan.virtio,
            virtio_count: scan.virtio_count,
            bootargs: scan.bootargs,
            // Only a v3 layout is understood; a v2's second range is a
            // different device.
            gic: scan.gic_v3.then_some(scan.gic_reg).flatten(),
            blob: (base, size),
        };
        Self {
            regions: reserve(&machine.regions, base, size, MemoryKind::Reclaimable),
            ..machine
        }
    }
}
