//! The MLOS hardware abstraction layer: one trait, declarations only.
//!
//! Invariant: nothing above this crate names a page size, a privilege
//! level, an interrupt controller, or an atomics width. Design and
//! history: docs/notes/mlos-hal.md.

#![no_std]
// Not `forbid(unsafe_code)`: `PageTable`'s methods are `unsafe fn`
// declarations. There are no `unsafe` blocks in this crate.

mod boot;
mod page;

pub use boot::{BootInfo, MemoryKind, MemoryRegion};
pub use page::{MapError, PageFlags, PageTable, PhysAddr, VirtAddr};

use mlos_device::{Console, IrqController, Timer};

/// Everything the kernel needs from the machine underneath it, held as
/// one value so a test can substitute one that touches no hardware.
pub trait Platform {
    /// This architecture's page tables. Static dispatch: mapping is on
    /// the fault path.
    type PageTable: PageTable;

    /// What the loader and firmware told us about this machine.
    fn boot_info(&self) -> &BootInfo<'_>;

    /// The console.
    fn console(&self) -> &dyn Console;

    /// The monotonic timer.
    fn timer(&self) -> &dyn Timer;

    /// The interrupt controller.
    fn irq(&self) -> &dyn IrqController;
}
