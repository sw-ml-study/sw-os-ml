//! Creating and destroying sessions.
//!
//! Invariant: an id names at most one live session, and a full table
//! refuses rather than evicting a session to make room. Design:
//! docs/notes/mlos-session.md.

use mlos_abi::{Error, Result};
use mlos_objtab::SessionId;

use crate::{Contract, Delivered, Session, Sessions};

impl<const N: usize> Sessions<N> {
    /// `ml_session_create`: a new session under `contract`, with the
    /// lowest free id from one upward. `Refused` when every slot is
    /// taken, which is the normal outcome admission control produces.
    pub fn create(&mut self, contract: Contract) -> Result<SessionId> {
        let id = (1..=u16::MAX)
            .map(SessionId)
            .find(|id| self.find(*id).is_none())
            .ok_or(Error::Refused)?;
        self.adopt(id, contract)?;
        Ok(id)
    }

    /// A session with an id the caller chose: what a replayed trace
    /// needs, since its sessions already have numbers. `Refused` if the
    /// id is taken, zero, or no slot is free.
    pub fn adopt(&mut self, id: SessionId, contract: Contract) -> Result<()> {
        if id.0 == 0 || self.find(id).is_some() {
            return Err(Error::Refused);
        }
        let slot = self
            .slots
            .iter_mut()
            .find(|s| s.is_none())
            .ok_or(Error::Refused)?;
        *slot = Some(Session {
            id,
            contract,
            resident: 0,
            delivered: Delivered::default(),
        });
        Ok(())
    }

    /// `ml_session_destroy`: forgets the session, returning its record so
    /// the caller can see what it still held. `BadObject` if unknown.
    pub fn destroy(&mut self, id: SessionId) -> Result<Session> {
        let at = self.find(id).ok_or(Error::BadObject)?;
        self.slots[at].take().ok_or(Error::BadObject)
    }

    /// The slot holding `id`.
    pub(crate) fn find(&self, id: SessionId) -> Option<usize> {
        self.slots
            .iter()
            .position(|s| s.is_some_and(|s| s.id == id))
    }
}
