//! Monotonic time, and the interrupt that marks its passing.

/// A count from a monotonic timer. Free-running, never adjusted, and
/// meaningless without the [`Hertz`] that goes with it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct Ticks(pub u64);

/// A frequency in hertz. Read from the platform at boot rather than
/// assumed: the aarch64 generic timer's rate is a board property, and
/// hard-coding it is a classic way to get a kernel that boots on one
/// machine and hangs on the next.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Hertz(pub u32);

/// The platform's monotonic timer.
pub trait Timer {
    /// The current tick count.
    fn now(&self) -> Ticks;

    /// How many ticks make a second.
    fn frequency(&self) -> Hertz;

    /// Arms an interrupt for the given tick count.
    ///
    /// Absolute, not a duration, because a duration has to be added to
    /// "now" by someone, and doing that in the caller races with the
    /// timer advancing between the read and the write.
    fn set_deadline(&self, at: Ticks);
}
