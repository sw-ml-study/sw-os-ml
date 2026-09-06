//! Proves the HAL is implementable and object-safe.
//!
//! A trait-only crate compiles whether or not anyone can implement it, so
//! the useful test is a whole platform that touches no hardware. This one
//! is also a rehearsal: the host-side simulator in `docs/design.md` s.10
//! needs exactly this shape to replay traces without a VM.

use core::cell::RefCell;

use mlos_device::{Console, Hertz, Irq, IrqController, Ticks, Timer};
use mlos_hal::{
    BootInfo, MapError, MemoryKind, MemoryRegion, PageFlags, PageTable, PhysAddr, Platform,
    VirtAddr,
};

/// A plausible `virt`-shaped memory map: firmware low, kernel at the
/// 2 MiB image base, the rest allocatable, plus one MMIO page.
static REGIONS: &[MemoryRegion] = &[
    MemoryRegion {
        base: 0x0900_0000,
        len: 0x0000_1000,
        kind: MemoryKind::Device,
    },
    MemoryRegion {
        base: 0x4000_0000,
        len: 0x0020_0000,
        kind: MemoryKind::Reclaimable,
    },
    MemoryRegion {
        base: 0x4020_0000,
        len: 0x0010_0000,
        kind: MemoryKind::Kernel,
    },
    MemoryRegion {
        base: 0x4030_0000,
        len: 0x1FD0_0000,
        kind: MemoryKind::Usable,
    },
];

#[derive(Default)]
struct NullConsole(RefCell<Vec<u8>>);
impl Console for NullConsole {
    fn write(&self, bytes: &[u8]) {
        self.0.borrow_mut().extend_from_slice(bytes);
    }
}

#[derive(Default)]
struct NullTimer(RefCell<u64>);
impl Timer for NullTimer {
    fn now(&self) -> Ticks {
        Ticks(*self.0.borrow())
    }
    fn frequency(&self) -> Hertz {
        Hertz(24_000_000)
    }
    fn set_deadline(&self, at: Ticks) {
        *self.0.borrow_mut() = at.0;
    }
}

#[derive(Default)]
struct NullIrq(RefCell<Option<Irq>>);
impl IrqController for NullIrq {
    fn enable(&self, irq: Irq) {
        *self.0.borrow_mut() = Some(irq);
    }
    fn claim(&self) -> Option<Irq> {
        *self.0.borrow()
    }
    fn complete(&self, _irq: Irq) {
        *self.0.borrow_mut() = None;
    }
}

#[derive(Default)]
struct NullTables;
impl PageTable for NullTables {
    unsafe fn map(
        &mut self,
        _va: VirtAddr,
        _pa: PhysAddr,
        len: usize,
        _flags: PageFlags,
    ) -> Result<(), MapError> {
        if len % 4096 == 0 {
            Ok(())
        } else {
            Err(MapError::Misaligned)
        }
    }
    unsafe fn unmap(&mut self, _va: VirtAddr, _len: usize) -> Result<(), MapError> {
        Ok(())
    }
    unsafe fn activate(&self) {}
}

#[derive(Default)]
struct NullPlatform {
    console: NullConsole,
    timer: NullTimer,
    irq: NullIrq,
}

impl Platform for NullPlatform {
    type PageTable = NullTables;

    fn boot_info(&self) -> &BootInfo<'_> {
        static INFO: BootInfo<'static> = BootInfo {
            regions: REGIONS,
            cpu_count: 4,
        };
        &INFO
    }
    fn console(&self) -> &dyn Console {
        &self.console
    }
    fn timer(&self) -> &dyn Timer {
        &self.timer
    }
    fn irq(&self) -> &dyn IrqController {
        &self.irq
    }
}

/// The whole platform is implementable, and its device accessors really
/// do hand back trait objects -- which is what makes a test or the
/// simulator able to substitute one.
#[test]
fn a_platform_with_no_hardware_satisfies_the_trait() {
    let platform = NullPlatform::default();

    platform.console().write(b"mlos");
    platform.irq().enable(Irq(30));
    platform.timer().set_deadline(Ticks(1_000));

    assert_eq!(platform.irq().claim(), Some(Irq(30)));
    assert_eq!(platform.timer().now(), Ticks(1_000));
    assert_eq!(platform.timer().frequency(), Hertz(24_000_000));
    assert_eq!(platform.boot_info().cpu_count, 4);
}

/// Only `Usable` counts. Reclaimable memory is real and worth taking
/// back, but it is not allocatable until someone has parsed what is in
/// it, so it must not inflate a residency budget.
#[test]
fn usable_bytes_ignores_kernel_device_and_reclaimable() {
    let platform = NullPlatform::default();
    assert_eq!(platform.boot_info().usable_bytes(), 0x1FD0_0000);
}

/// The associated type is a concrete implementation, not a trait object:
/// mapping is on the model-fault path and must not cost a virtual call.
#[test]
fn page_tables_are_usable_through_the_associated_type() {
    let mut tables = <NullPlatform as Platform>::PageTable::default();

    // SAFETY: NullTables maps nothing; there is no real aliasing to
    // reason about, which is the point of a null implementation.
    let aligned = unsafe { tables.map(VirtAddr(0), PhysAddr(0), 8192, PageFlags::default()) };
    // SAFETY: as above.
    let ragged = unsafe { tables.map(VirtAddr(0), PhysAddr(0), 100, PageFlags::default()) };

    assert_eq!(aligned, Ok(()));
    assert_eq!(ragged, Err(MapError::Misaligned));
}
