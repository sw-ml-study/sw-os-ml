//! What one policy did with one trace.
//!
//! Invariant: every field is a count the replay incremented; nothing
//! here is derived until asked. Design: docs/notes/mlos-sim.md.

use mlos_metrics::Headline;

/// What one policy did with one trace.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Outcome {
    /// Acquires that found the object already resident. The good case.
    pub hits: u64,
    /// Acquires that had to go to a provider. The number to minimise.
    pub reads: u64,
    /// Bytes those reads moved.
    pub bytes: u64,
    /// Objects thrown away to make room.
    pub evicted: u64,
    /// Acquires that could not be served at all. Part of the cost, not a
    /// failure.
    pub refused: u64,
    /// What the reads would have cost, in nanoseconds, as providers charge.
    pub cost: u64,
    /// Accesses naming objects this model does not have. Zero for a
    /// matched trace and model; non-zero means the two files disagree.
    pub mismatched: u64,
    /// Distinct sessions the trace named, each created for the replay.
    pub sessions: u64,
    /// Weight bytes served, by hit or by read: parameter applications.
    pub weights_applied: u64,
    /// Weight bytes read from a provider.
    pub weights_read: u64,
    /// KV bytes resident when the replay ended.
    pub kv_resident: u64,
}

impl Outcome {
    /// Hits per thousand accesses that were served or refused.
    #[must_use]
    pub const fn hit_per_mille(&self) -> u64 {
        match self.hits + self.reads + self.refused {
            0 => 0,
            asked => self.hits * 1000 / asked,
        }
    }

    /// `Ps`, `Ks` and `Ss` for this replay under `budget` bytes, computed
    /// by `mlos-metrics` exactly as the kernel computes its own.
    #[must_use]
    pub const fn headline(&self, budget: u64) -> Headline {
        let sessions = if self.sessions > u32::MAX as u64 {
            u32::MAX
        } else {
            self.sessions as u32
        };
        Headline::of(
            self.weights_applied,
            self.weights_read,
            self.kv_resident,
            sessions,
            budget,
        )
    }
}
