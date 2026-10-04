//! Admission control: whether a budget can honour one more contract
//! beside the ones it already admitted.
//!
//! Invariant: refusing is a normal outcome and says why; a contract the
//! budget admits is one it can honour in bytes and in latency against
//! every contract already live. Design and history:
//! docs/notes/mlos-admit.md.

#![no_std]
#![forbid(unsafe_code)]

mod latency;
mod refusal;

use mlos_session::{Contract, Sessions};

pub use refusal::Refusal;

/// What a budget has to give, and what one token costs against it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Capacity {
    /// Resident bytes the budget holds. Zero: nothing is tested in bytes.
    pub bytes: u64,
    /// Bytes no session may claim: the per-token working set of the
    /// weights, which every session needs resident in its turn.
    pub reserved: u64,
    /// Bytes of KV one token of context adds, over every layer.
    pub kv_per_token: u64,
    /// Acquires one token takes in weights, which is what a latency
    /// ceiling is measured against. Zero: nothing is tested in latency.
    pub token: u32,
}

impl Capacity {
    /// A budget that admits anything: what a replayed trace runs under.
    pub const NONE: Self = Self {
        bytes: 0,
        reserved: 0,
        kv_per_token: 0,
        token: 0,
    };

    /// The bytes `contract` will hold: the KV of its declared context.
    #[must_use]
    pub const fn need(&self, contract: &Contract) -> u64 {
        (contract.context as u64).saturating_mul(self.kv_per_token)
    }

    /// Bytes not yet promised to the sessions in `live`.
    fn free<const N: usize>(&self, live: &Sessions<N>) -> u64 {
        let promised = live
            .each()
            .map(|s| self.need(&s.contract))
            .fold(0u64, u64::saturating_add);
        self.bytes
            .saturating_sub(self.reserved)
            .saturating_sub(promised)
    }

    /// Whether `contract` can be honoured beside everything in `live`:
    /// a slot, its own ceiling above its context, the bytes unpromised,
    /// and the latency rules in `latency.rs`.
    pub fn admit<const N: usize>(
        &self,
        live: &Sessions<N>,
        contract: &Contract,
    ) -> Result<(), Refusal> {
        if live.live() as usize >= N {
            return Err(Refusal::Slots);
        }
        let need = self.need(contract);
        let ceiling = contract.resident_ceiling;
        if ceiling != 0 && need > ceiling {
            return Err(Refusal::Ceiling { ceiling, need });
        }
        let free = self.free(live);
        if self.bytes != 0 && need > free {
            return Err(Refusal::Bytes { need, free });
        }
        self.wait(live, contract)
    }

    /// `Ss` as a capacity figure: how many sessions like the live ones
    /// the budget admits, at the bytes they declared. The live count
    /// itself when nothing declared a context, which is what M4 measured.
    #[must_use]
    pub fn admits<const N: usize>(&self, live: &Sessions<N>) -> u32 {
        let promised = live
            .each()
            .map(|s| self.need(&s.contract))
            .fold(0u64, u64::saturating_add);
        let room = self.bytes.saturating_sub(self.reserved);
        match room
            .saturating_mul(u64::from(live.live()))
            .checked_div(promised)
        {
            Some(fit) if self.bytes != 0 && promised != 0 => u32::try_from(fit).unwrap_or(u32::MAX),
            _ => live.live(),
        }
    }
}
