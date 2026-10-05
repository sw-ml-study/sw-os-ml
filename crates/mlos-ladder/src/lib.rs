//! The degradation ladder: what each rung does to a session's cache, and
//! the walk down it when the budget shrinks.
//!
//! Invariant: one function, `Shape::fate`, decides what every block
//! becomes on every rung; what a rung needs and what it costs are sums
//! of it, so the plan, the table and the host's prediction cannot
//! disagree. Design and history: docs/notes/mlos-ladder.md.

#![no_std]
#![forbid(unsafe_code)]

mod fate;
mod squeeze;
mod walk;

pub use fate::Fate;
pub use mlos_session::Rung;
pub use squeeze::{Squeezed, squeeze};
pub use walk::{Move, arrive, movable, next};

/// How a session's context is cut into KV blocks, and how much of it is
/// hot: the newest blocks, which no rung but L5 touches.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Shape {
    /// Tokens per KV block. One is a block per position.
    pub per_block: u32,
    /// The newest blocks kept at full precision on every rung below L5.
    pub hot: u32,
}

impl Shape {
    /// Blocks a context of `context` tokens is cut into, the last one
    /// possibly partial.
    #[must_use]
    pub const fn blocks(&self, context: u32) -> u32 {
        context.div_ceil(if self.per_block == 0 {
            1
        } else {
            self.per_block
        })
    }

    /// Bytes block `at` holds at full precision over every layer, when a
    /// token costs `kv` bytes: a whole block, or what is left of the
    /// context in the last one.
    #[must_use]
    pub const fn full(&self, at: u32, context: u32, kv: u64) -> u64 {
        let per = if self.per_block == 0 {
            1
        } else {
            self.per_block
        };
        let start = at.saturating_mul(per);
        let tokens = context.saturating_sub(start);
        (if tokens < per { tokens } else { per }) as u64 * kv
    }

    /// Bytes a context of `context` tokens holds on `rung`. On L0 this is
    /// `context * kv`, what admission charged.
    #[must_use]
    pub fn need(&self, rung: Rung, context: u32, kv: u64) -> u64 {
        let blocks = self.blocks(context);
        (0..blocks)
            .map(|at| {
                self.fate(rung, at, blocks)
                    .bytes(self.full(at, context, kv))
            })
            .fold(0, u64::saturating_add)
    }

    /// Bytes read to move a context from `from` to `to`: every block that
    /// is requantized or summarised, at its size before. `Rc`'s share of
    /// the step.
    #[must_use]
    pub fn cost(&self, from: Rung, to: Rung, context: u32, kv: u64) -> u64 {
        let blocks = self.blocks(context);
        (0..blocks)
            .filter(|at| self.reads(from, to, *at, blocks))
            .map(|at| {
                self.fate(from, at, blocks)
                    .bytes(self.full(at, context, kv))
            })
            .fold(0, u64::saturating_add)
    }
}
