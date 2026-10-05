//! What one block becomes on one rung.
//!
//! Invariant: the hot window is full precision on every rung below L5,
//! and a block a rung drops stays dropped on every deeper rung. Design:
//! docs/notes/mlos-ladder.md.

use mlos_objtab::Precision;

use crate::{Rung, Shape};

/// What a rung makes of one block.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fate {
    /// Kept, at this precision.
    Keep(Precision),
    /// The first block of the oldest span, standing for the whole span:
    /// one block at Q4.
    Summary,
    /// Gone: summarised into the first block, cut as a candidate, or
    /// truncated.
    Dropped,
}

impl Fate {
    /// The precision it is held at, or `None` if it is not held.
    #[must_use]
    pub const fn precision(self) -> Option<Precision> {
        match self {
            Self::Keep(precision) => Some(precision),
            Self::Summary => Some(Precision::Q4),
            Self::Dropped => None,
        }
    }

    /// Bytes it holds, for a block of `full` bytes at full precision.
    #[must_use]
    pub const fn bytes(self, full: u64) -> u64 {
        let bits = match self.precision() {
            None => 0,
            Some(Precision::Fp16 | Precision::Bf16) => 16,
            Some(Precision::Q8) => 8,
            Some(Precision::Q4) => 4,
            Some(Precision::Q3) => 3,
            Some(Precision::Ternary) => 2,
        };
        full * bits / 16
    }
}

impl Shape {
    /// What block `at` of `blocks` (zero oldest) is on `rung`. Cold is
    /// everything older than the hot window; the oldest span is the
    /// older half of the cold blocks, when that is two or more; the
    /// candidates are what is cold and not in the span, and L4 keeps the
    /// newer half of them.
    #[must_use]
    pub const fn fate(&self, rung: Rung, at: u32, blocks: u32) -> Fate {
        let cold = blocks.saturating_sub(self.hot);
        let span = if cold / 2 >= 2 { cold / 2 } else { 0 };
        // The older half of the candidates, which L4 drops.
        let cut = (cold - span) - (cold - span) / 2;
        match rung {
            _ if at >= cold => Fate::Keep(Precision::Fp16),
            Rung::L0 => Fate::Keep(Precision::Fp16),
            Rung::L1 => Fate::Keep(Precision::Q8),
            Rung::L2 => Fate::Keep(Precision::Q4),
            Rung::L3 | Rung::L4 if at == 0 && span != 0 => Fate::Summary,
            Rung::L3 | Rung::L4 if at < span => Fate::Dropped,
            Rung::L4 if at < span + cut => Fate::Dropped,
            Rung::L3 | Rung::L4 => Fate::Keep(Precision::Q4),
            _ => Fate::Dropped,
        }
    }

    /// Whether moving from `from` to `to` reads block `at`: it changes
    /// form and is still held, or it is part of the span a summary is
    /// made from on this move. Truncation reads nothing.
    #[must_use]
    pub const fn reads(&self, from: Rung, to: Rung, at: u32, blocks: u32) -> bool {
        let before = self.fate(from, at, blocks);
        let after = self.fate(to, at, blocks);
        match (before, after) {
            (Fate::Dropped, _) => false,
            (Fate::Keep(a), Fate::Keep(b)) => a as u8 != b as u8,
            (Fate::Summary, Fate::Summary) => false,
            (_, Fate::Dropped) => {
                (from as u8) < Rung::L3 as u8
                    && (to as u8) < Rung::L5 as u8
                    && matches!(self.fate(Rung::L3, at, blocks), Fate::Dropped)
            }
            _ => true,
        }
    }
}
