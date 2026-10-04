//! Beginning and ending a session: admission on the way in, and what it
//! owned going with it on the way out.
//!
//! Invariant: a session exists only if the capacity admitted it, and a
//! destroyed one leaves no object of its own in the table, resident or
//! not; if one of them cannot be evicted, nothing is destroyed and the
//! session stays. Design: docs/notes/mlos-session.md.

use mlos_abi::Result;
use mlos_objtab::SessionId;

use crate::{Contract, Manager, Refusal};

impl<const N: usize> Manager<'_, N> {
    /// `ml_session_create`: a session under `contract` if the capacity
    /// can honour it beside the live ones, else why not.
    pub fn create_session(
        &mut self,
        contract: Contract,
    ) -> core::result::Result<SessionId, Refusal> {
        self.capacity.admit(&self.sessions, &contract)?;
        self.sessions.create(contract).map_err(|_| Refusal::Slots)
    }

    /// `ml_session_destroy`: evicts every resident object the session
    /// owns, forgets the rest, and forgets the session. Returns how many
    /// objects were evicted. Fails, changing nothing further, if an
    /// eviction is refused.
    pub fn destroy_session(&mut self, id: SessionId) -> Result<u32> {
        let mut evicted = 0;
        for slot in 0..N {
            let Some((object, meta)) = self.table.at(slot) else {
                continue;
            };
            if meta.owner != id || id.0 == 0 {
                continue;
            }
            if meta.resident_at != 0 {
                self.evict(object)?;
                evicted += 1;
            }
            self.table.remove(object);
        }
        self.sessions.destroy(id)?;
        Ok(evicted)
    }

    /// Destroys every session, as a replay does when it ends. Returns how
    /// many objects went with them; a session that refuses is skipped.
    pub fn destroy_all_sessions(&mut self) -> u32 {
        let mut evicted = 0;
        for slot in 0..mlos_session::MAX_SESSIONS {
            let Some(id) = self.sessions.at(slot).map(|s| s.id) else {
                continue;
            };
            evicted += self.destroy_session(id).unwrap_or(0);
        }
        evicted
    }
}

impl<const N: usize> Manager<'_, N> {
    /// `Ps`, `Ks` and `Ss` right now: the counters' headline over the
    /// sessions the budget admits (the live count, until one declares a
    /// context) and the arena's capacity.
    #[must_use]
    pub fn headline(&self) -> mlos_metrics::Headline {
        let sessions = self.capacity.admits(&self.sessions);
        let budget = self.arena.occupancy().capacity;
        self.counters.report().headline(sessions, budget)
    }
}
