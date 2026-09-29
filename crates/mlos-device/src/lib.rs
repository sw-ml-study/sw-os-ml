//! Device traits an MLOS platform implements.
//!
//! Invariant: nothing here names a page size, a privilege level, a
//! specific interrupt controller, or an atomics width. Design and
//! history: docs/notes/mlos-device.md.

#![no_std]
#![forbid(unsafe_code)]

mod console;
mod irq;
mod timer;

pub use console::Console;
pub use irq::{Irq, IrqController};
pub use timer::{Hertz, Ticks, Timer};
