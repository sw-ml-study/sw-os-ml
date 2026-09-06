//! Device traits an MLOS platform implements.
//!
//! Split out of `mlos-hal` because devices and platforms are different
//! kinds of thing: a console or an interrupt controller is discoverable
//! and pluggable, while the platform is singular and fixed at boot. The
//! `sw-checklist` module gate is what forced the question, but the answer
//! stands on its own -- see AGENTS.md, "design the split BEFORE you need
//! it".
//!
//! Nothing here names a page size, a privilege level, a specific
//! interrupt controller, or an atomics width. That restriction is what
//! keeps a future `mlos-hal-riscv64` an additive change rather than a
//! refactor (`docs/architecture.md` s.9).

#![no_std]
#![forbid(unsafe_code)]

mod console;
mod irq;
mod timer;

pub use console::Console;
pub use irq::{Irq, IrqController};
pub use timer::{Hertz, Ticks, Timer};
