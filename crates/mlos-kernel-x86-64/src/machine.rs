//! The machine, from what the PVH loader handed over.
//!
//! The x86-64 counterpart of `mlos_machine::Machine::probe`: the memory
//! map from `hvm_start_info`, the virtio-mmio slots and `mlsh.run=` from
//! the command line (`mlos-pvh`). The kernel image and the loader's own
//! structures are carved out of the map with the same `reserve` the
//! aarch64 path uses, so `mem` reports what was found on both.

use core::sync::atomic::AtomicU32;

use mlos_hal::{BootInfo, MemoryKind};
use mlos_hal_x86_64 as hal;
use mlos_machine::{Regions, reserve};
use mlos_pvh::{ENTRY_LEN, HEADER_LEN, StartInfo};
use mlos_uart16550::{COM1, Uart16550};
use mlsh::{Facts, Shell};

/// COM1's interrupt line: IRQ 4, by PC convention since the 8250 rather
/// than by discovery -- nothing short of ACPI would say otherwise.
const COM1_IRQ: u32 = 4;

/// Timer ticks. Stays zero until step `x86-interrupts` gives the guest
/// a timer; `ticks` in the shell says so by reporting it.
static TICKS: AtomicU32 = AtomicU32::new(0);

/// What was found.
pub struct Machine {
    /// The memory map, with the image and loader structures carved out.
    pub regions: Regions,
    /// RAM before anything was carved out of it.
    pub total: u64,
    /// The command line as the user gave it -- without the entries QEMU
    /// appended -- which is what `mlsh.run=` reads to its end.
    pub cmdline: &'static str,
    /// Lowest virtio-mmio slot, its size, and how many there are.
    pub virtio: Option<(usize, usize, u32)>,
    /// Memory-map records that did not fit in `Regions`. Reported, never
    /// silently lost: a dropped record is memory the kernel does not know
    /// about.
    pub dropped: usize,
}

/// Reads the machine from the PVH start info, or `None` if there is no
/// valid one -- in which case there is no memory map to run on.
///
/// # Safety
///
/// `start_info` must be what the PVH entry received in `%ebx`.
pub unsafe fn discover(start_info: *const u8) -> Option<Machine> {
    // SAFETY: the loader's structures, never written by the kernel.
    let info = StartInfo::parse(unsafe { hal::bytes(start_info as u64, HEADER_LEN) })?;
    let map_len = info.memmap_entries as usize * ENTRY_LEN;
    // SAFETY: as above.
    let (map, cmdline) = unsafe { (hal::bytes(info.memmap, map_len), hal::c_str(info.cmdline)) };
    let (mut regions, mut dropped) = (Regions::default(), 0);
    mlos_pvh::regions(map, |region| dropped += usize::from(!regions.push(region)));
    let usable = regions
        .as_slice()
        .iter()
        .filter(|r| r.kind == MemoryKind::Usable);
    let total = usable.map(|r| r.len).sum();
    // What occupies RAM the map calls free: the image, and the loader's
    // own structures -- reclaimable once read, like the aarch64 DTB.
    let (image, reclaim) = (hal::extent(), MemoryKind::Reclaimable);
    let carve = [
        (image.0, image.1, MemoryKind::Kernel),
        (start_info as u64, HEADER_LEN as u64, reclaim),
        (info.memmap, map_len as u64, reclaim),
        (info.cmdline, cmdline.len() as u64 + 1, reclaim),
    ];
    // Only out of Usable: `reserve` re-labels whatever it overlaps, and
    // QEMU puts the loader's structures in the BIOS area when ACPI is off,
    // which the map already calls Reserved. Re-labelling that Reclaimable
    // would one day hand firmware memory to an allocator.
    for (base, len, kind) in carve {
        if free(&regions, base, len) {
            regions = reserve(&regions, base, len, kind);
        }
    }
    let virtio = mlos_pvh::virtio_slots(cmdline);
    let cmdline = mlos_pvh::user_args(cmdline);
    Some(Machine {
        regions,
        total,
        cmdline,
        virtio,
        dropped,
    })
}

/// True if `[base, base + len)` lies wholly inside one Usable region.
fn free(regions: &Regions, base: u64, len: u64) -> bool {
    let inside = |r: &&mlos_hal::MemoryRegion| base >= r.base && base + len <= r.base + r.len;
    regions
        .as_slice()
        .iter()
        .filter(|r| r.kind == MemoryKind::Usable)
        .any(|r| inside(&r))
}

impl Machine {
    /// Hands the machine to `mlsh` on `console`, forever.
    pub fn run_shell(&self, console: &mut Uart16550, idle: fn()) -> ! {
        if let Some((base, size, count)) = self.virtio {
            mlos_lab::set_slots(base, size, count);
        }
        let facts = Facts {
            info: BootInfo::new(self.regions.as_slice(), 1),
            total: self.total,
            image: hal::extent(),
            uart: usize::from(COM1),
            uart_irq: COM1_IRQ,
            timer_irq: 0,
            gic: None,
            console: "16550",
            virtio: self.virtio.map(|(base, size, _)| (base, size)),
            virtio_count: self.virtio.map_or(0, |(_, _, count)| count),
            clock: (|| 0, 0),
            bootargs: self.cmdline,
            ticks: &TICKS,
        };
        Shell::default().run(console, &facts, idle)
    }
}
