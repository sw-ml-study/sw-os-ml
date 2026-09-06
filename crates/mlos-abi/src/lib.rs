//! The MLOS ABI.
//!
//! The only crate both the kernel and userspace depend on. Everything that
//! crosses the syscall boundary is defined here, with a fixed layout, and
//! is tested for that layout rather than trusted to stay put.
//!
//! It is `no_std` and `forbid(unsafe_code)`: an ABI definition that needs
//! `unsafe` to describe itself is an ABI that will be got wrong somewhere.

#![no_std]
#![forbid(unsafe_code)]

mod class;
mod error;
mod object;

pub use class::ObjectClass;
pub use error::{Error, Result};
pub use object::{Fields, ObjectId};
