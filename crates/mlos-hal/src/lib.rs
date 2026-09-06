//! The MLOS hardware abstraction layer.
//!
//! One trait, four methods, and a hard rule: **nothing above this crate
//! may name a page size, a privilege level, a specific interrupt
//! controller, or an atomics width.** That is not tidiness. It is the
//! concession that keeps `mlos-hal-riscv64` an additive change rather
//! than a refactor when RISC-V's GPU support matures
//! (`docs/architecture.md` s.9).
//!
//! The trait is small on purpose. A wide HAL is a HAL that has started
//! leaking architecture into the kernel, and the leak is always found
//! later than the widening.
//!
//! This crate is declarations only. Implementations live in
//! `mlos-hal-aarch64` and `mlos-hal-x86-64`, which is where `unsafe`
//! belongs.

#![no_std]
// Not `forbid(unsafe_code)`: `PageTable`'s methods are `unsafe fn`
// declarations, and they should be -- installing a mapping can alias
// memory arbitrarily, and nothing here can check the caller's claim.
// There are no `unsafe` *blocks* in this crate; it is declarations only.

mod boot;
mod page;

pub use boot::{BootInfo, MemoryKind, MemoryRegion};
pub use page::{MapError, PageFlags, PageTable, PhysAddr, VirtAddr};

use mlos_device::{Console, IrqController, Timer};

/// Everything the kernel needs from the machine underneath it.
///
/// Held by the kernel as a single value, so a test or the host-side
/// simulator can substitute one that touches no hardware at all.
pub trait Platform {
    /// This architecture's page tables.
    ///
    /// An associated type rather than a `dyn` object: mapping is on the
    /// model-fault path, and a virtual call per mapping is a cost the
    /// fault budget in `docs/architecture.md` s.4 will not stand.
    type PageTable: PageTable;

    /// What the loader and firmware told us about this machine.
    fn boot_info(&self) -> &BootInfo<'_>;

    /// The console. `dyn` is fine here: printing is never hot.
    fn console(&self) -> &dyn Console;

    /// The monotonic timer.
    fn timer(&self) -> &dyn Timer;

    /// The interrupt controller.
    fn irq(&self) -> &dyn IrqController;
}
