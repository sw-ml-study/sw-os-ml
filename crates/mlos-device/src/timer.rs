//! Monotonic time, and the interrupt that marks its passing. Design:
//! docs/notes/mlos-device.md.

/// A count from a monotonic timer. Free-running, never adjusted, and
/// meaningless without the [`Hertz`] that goes with it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct Ticks(pub u64);

/// A frequency in hertz, read from the platform at boot, never assumed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Hertz(pub u32);

/// The platform's monotonic timer.
pub trait Timer {
    /// The current tick count.
    fn now(&self) -> Ticks;

    /// How many ticks make a second.
    fn frequency(&self) -> Hertz;

    /// Arms an interrupt for the given absolute tick count.
    fn set_deadline(&self, at: Ticks);
}
