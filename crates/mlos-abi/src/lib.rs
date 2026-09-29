//! The MLOS ABI: everything that crosses the syscall boundary.
//!
//! Invariant: layouts and discriminants are fixed, tested, and never
//! renumbered. Design and history: docs/notes/mlos-abi.md.

#![no_std]
#![forbid(unsafe_code)]

mod class;
mod error;
mod object;

pub use class::ObjectClass;
pub use error::{Error, Result};
pub use object::{Fields, ObjectId};
