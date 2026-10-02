//! The two headline numbers of `docs/PRD.md` s.5.2, and the one that
//! comes with them, computed in one place so the kernel and the
//! simulator cannot disagree about what they mean.
//!
//! Invariant: pure arithmetic over counts, integer, with zero
//! denominators answering zero. Design: docs/notes/mlos-metrics.md.

use mlos_abi::ObjectClass;

use crate::Report;

/// `Ps`, `Ss` and `Ks`, as fixed-point integers.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Headline {
    /// `Ps`: weight bytes applied per thousand weight bytes read from a
    /// provider. One thousand means every weight read served one use;
    /// four thousand means four sessions shared each read.
    pub ps_per_mille: u32,
    /// `Ks`: KV bytes resident per live session.
    pub ks: u64,
    /// `Ss`: live sessions per GiB of resident budget, in thousandths.
    pub ss_milli: u64,
}

impl Headline {
    /// From the raw counts: weight bytes applied (hits and reads) and
    /// read, KV bytes resident, live sessions, and the budget in bytes.
    #[must_use]
    pub const fn of(applied: u64, read: u64, kv: u64, sessions: u32, budget: u64) -> Self {
        let ps = match applied.saturating_mul(1000).checked_div(read) {
            Some(ps) => ps,
            None => 0,
        };
        let ss = (sessions as u64).saturating_mul(1000 << 30);
        Self {
            ps_per_mille: if ps > u32::MAX as u64 {
                u32::MAX
            } else {
                ps as u32
            },
            ks: match kv.checked_div(sessions as u64) {
                Some(ks) => ks,
                None => 0,
            },
            ss_milli: match ss.checked_div(budget) {
                Some(ss) => ss,
                None => 0,
            },
        }
    }
}

impl core::fmt::Display for Headline {
    /// The form both count lines carry, so the kernel's and the
    /// simulator's can be compared as strings.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "ps {}/1000, ks {} B, ss {}/1000 per GiB",
            self.ps_per_mille, self.ks, self.ss_milli
        )
    }
}

impl Report {
    /// The headline numbers from this report, for `sessions` live sessions
    /// over a `budget` of bytes.
    #[must_use]
    pub const fn headline(&self, sessions: u32, budget: u64) -> Headline {
        let weights = ObjectClass::WeightTile.index();
        let kv = ObjectClass::KvBlock.index();
        let read = self.fetched[weights];
        Headline::of(
            read.saturating_add(self.hits[weights]),
            read,
            self.held[kv],
            sessions,
            budget,
        )
    }
}
