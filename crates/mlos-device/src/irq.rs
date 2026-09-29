//! Interrupt delivery, without naming a controller. Design:
//! docs/notes/mlos-device.md.

/// An interrupt number, as the platform numbers them. Opaque above the
/// HAL: code gets one from device discovery and passes it back, and must
/// not do arithmetic on it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Irq(pub u32);

/// The platform's interrupt controller.
pub trait IrqController {
    /// Routes an interrupt to this CPU and unmasks it.
    fn enable(&self, irq: Irq);

    /// Takes the highest-priority pending interrupt, if any. Every claim
    /// must be followed by a [`Self::complete`], or the controller
    /// withholds the next interrupt at that priority.
    fn claim(&self) -> Option<Irq>;

    /// Signals that the handler for a claimed interrupt has finished.
    fn complete(&self, irq: Irq);
}
