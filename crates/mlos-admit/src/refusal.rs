//! Why a contract was not admitted, in words a shell can print.
//!
//! Invariant: every refusal names the figure that failed and the figure
//! it was held to. Design: docs/notes/mlos-admit.md.

use core::fmt;

/// Why admission said no. Not an error: `docs/design.md` s.5.3 calls
/// refusing a normal outcome.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Refusal {
    /// Every session slot is taken.
    Slots,
    /// The contract's own resident ceiling is below what its context
    /// will hold, so it could never be honoured even alone.
    Ceiling {
        /// Its resident ceiling, in bytes.
        ceiling: u64,
        /// What its context needs, in bytes.
        need: u64,
    },
    /// The budget has fewer unpromised bytes than the context needs.
    Bytes {
        /// What its context needs, in bytes.
        need: u64,
        /// What the budget still had, in bytes.
        free: u64,
    },
    /// A latency ceiling, this one's or one already promised, that could
    /// not be kept with this session admitted.
    Latency {
        /// The ceiling that would be broken, in acquires.
        ceiling: u32,
        /// The least ceiling that could be kept, in acquires.
        least: u32,
    },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Slots => write!(f, "no session slot free"),
            Self::Ceiling { ceiling, need } => write!(
                f,
                "its context needs {need} B resident, over its own ceiling of {ceiling} B"
            ),
            Self::Bytes { need, free } => {
                write!(
                    f,
                    "its context needs {need} B, the budget has {free} B unpromised"
                )
            }
            Self::Latency { ceiling, least } => write!(
                f,
                "a ceiling of {ceiling} acquires could not be kept; the least here is {least}"
            ),
        }
    }
}

impl From<Refusal> for mlos_abi::Error {
    fn from(_: Refusal) -> Self {
        Self::Refused
    }
}
