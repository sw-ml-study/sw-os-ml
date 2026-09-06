//! Interrupt delivery, without naming a controller.

/// An interrupt number, as the platform numbers them.
///
/// Deliberately opaque above the HAL. On aarch64 these are GIC INTIDs
/// with SGI/PPI/SPI ranges; on x86-64 they are APIC vectors. Code above
/// `mlos-hal` gets them from device discovery and passes them back, and
/// must not do arithmetic on them.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Irq(pub u32);

/// The platform's interrupt controller.
pub trait IrqController {
    /// Routes an interrupt to this CPU and unmasks it.
    fn enable(&self, irq: Irq);

    /// Takes the highest-priority pending interrupt, if any.
    ///
    /// Claiming and completing are separate because the controller needs
    /// to know the handler has finished before it will deliver another at
    /// the same priority. Merging them would silently drop nested
    /// interrupts.
    fn claim(&self) -> Option<Irq>;

    /// Signals that the handler for a claimed interrupt has finished.
    fn complete(&self, irq: Irq);
}
