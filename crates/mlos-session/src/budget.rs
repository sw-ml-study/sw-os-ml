//! What a session holds, against what it was promised.
//!
//! Invariant: `charge` refuses before the bytes are placed, so a session
//! never exceeds its ceiling and nothing has to be undone. Objects owned
//! by session zero, or by a session the table does not know, are not
//! counted against anyone. Design: docs/notes/mlos-session.md.

use mlos_abi::{Error, Result};
use mlos_objtab::{Precision, SessionId};

use crate::{Session, Sessions};

impl<const N: usize> Sessions<N> {
    /// The session with `id`.
    #[must_use]
    pub fn get(&self, id: SessionId) -> Option<&Session> {
        self.find(id).and_then(|at| self.slots[at].as_ref())
    }

    /// The session with `id`, to change.
    pub fn get_mut(&mut self, id: SessionId) -> Option<&mut Session> {
        self.find(id).and_then(|at| self.slots[at].as_mut())
    }

    /// Counts `size` bytes at `precision` about to become resident for
    /// `owner`, and records what that delivers: the peak, and the coarsest
    /// precision served. `Refused` if the bytes would pass the owner's
    /// ceiling; then nothing is counted. Unknown owners are allowed and
    /// uncounted.
    pub fn charge(&mut self, owner: SessionId, size: u32, precision: Precision) -> Result<()> {
        let Some(session) = self.get_mut(owner) else {
            return Ok(());
        };
        let ceiling = session.contract.resident_ceiling;
        let after = session.resident + u64::from(size);
        if ceiling != 0 && after > ceiling {
            return Err(Error::Refused);
        }
        session.resident = after;
        let got = &mut session.delivered;
        got.peak_resident = got.peak_resident.max(after);
        if precision as u8 > got.coarsest as u8 {
            got.coarsest = precision;
        }
        Ok(())
    }

    /// Gives `size` bytes back to `owner`'s account after an eviction.
    pub fn credit(&mut self, owner: SessionId, size: u32) {
        if let Some(session) = self.get_mut(owner) {
            session.resident = session.resident.saturating_sub(u64::from(size));
        }
    }
}
