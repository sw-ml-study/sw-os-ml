//! The latency half of admission: what a ceiling costs the others.
//!
//! Invariant: the two rules are the G5 report's measurements, s.4,
//! stated as integer arithmetic. Design: docs/notes/mlos-admit.md.

use mlos_session::{Contract, Sessions};

use crate::{Capacity, Refusal};

impl Capacity {
    /// A ceiling below a quarter of a token puts the session a full
    /// token ahead, alone, and the others pay for every weight it reads
    /// (G5 s.4: eight times the reads). And with `k` contracted sessions
    /// all overdue at once, each waits for the `k - 1` tokens ahead of
    /// it, so every ceiling here must cover them.
    pub(crate) fn wait<const N: usize>(
        &self,
        live: &Sessions<N>,
        contract: &Contract,
    ) -> Result<(), Refusal> {
        let (token, ceiling) = (self.token, contract.latency_ceiling);
        if token == 0 || ceiling == 0 {
            return Ok(());
        }
        let least = token / 4;
        if ceiling < least {
            return Err(Refusal::Latency { ceiling, least });
        }
        let (tightest, ahead) = Self::promised(live, ceiling);
        let least = token.saturating_mul(ahead);
        if tightest < least {
            return Err(Refusal::Latency {
                ceiling: tightest,
                least,
            });
        }
        Ok(())
    }

    /// The tightest ceiling once `ceiling` joins the live ones, and how
    /// many contracted sessions are already here to run ahead of it.
    fn promised<const N: usize>(live: &Sessions<N>, ceiling: u32) -> (u32, u32) {
        let promised = || {
            live.each()
                .map(|s| s.contract.latency_ceiling)
                .filter(|c| *c != 0)
        };
        let tightest = promised().min().unwrap_or(u32::MAX).min(ceiling);
        (tightest, promised().count() as u32)
    }
}
