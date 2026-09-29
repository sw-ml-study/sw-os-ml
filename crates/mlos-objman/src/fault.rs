//! What a model fault says, and how a miss is serviced.
//!
//! Invariant: the fault is described before anything is attempted, and
//! bytes are fetched before the table calls the object resident. Design
//! and history: docs/notes/mlos-objman.md.

use mlos_abi::{Error, ObjectClass, ObjectId, Result};
use mlos_events::Event;
use mlos_objtab::{CostNs, NextUse, SessionId, Tier};
use mlos_provider::Located;

use mlos_objtab::ObjectMeta;
use mlos_provider::Provider;

use crate::{Handle, Lease, Manager};

/// A miss on the object table, described.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ModelFault {
    /// What kind of state was wanted.
    pub class: ObjectClass,
    /// Which model, or which session for a session-scoped class.
    pub model: u16,
    /// Which layer.
    pub layer: u16,
    /// Which tensor within the layer.
    pub tensor: u16,
    /// Which tile within the tensor.
    pub tile: u8,
    /// Who wanted it.
    pub session: SessionId,
    /// What fetching it will cost, as the provider reckons it.
    pub cost: CostNs,
}

impl ModelFault {
    /// Describes a miss on `id`. `None` if the id does not decode.
    #[must_use]
    pub fn new(id: ObjectId, session: SessionId, cost: CostNs) -> Option<Self> {
        let fields = id.fields();
        Some(Self {
            class: id.class()?,
            model: fields.model,
            layer: fields.layer,
            tensor: fields.tensor,
            tile: fields.tile,
            session,
            cost,
        })
    }
}

impl<'a, const N: usize> Manager<'a, N> {
    /// Services a miss: describe it, find the provider, make room, fetch.
    /// The fault is recorded before any attempt; the event after the
    /// outcome, with an arena-relative offset.
    pub(crate) fn service(
        &mut self,
        id: ObjectId,
        lease: Lease,
        by: SessionId,
        wanted: NextUse,
    ) -> Result<Handle> {
        let meta = *self.table.get(id).ok_or(Error::BadObject)?;
        let provider = self.provider_for(&meta)?;
        let located = Located::new(id, &meta);

        let cost = provider.cost(located).for_bytes(meta.size);
        let fault = ModelFault::new(id, by, cost).ok_or(Error::BadClass)?;
        self.last_fault = Some(fault);
        self.counters.fault(fault.class, meta.size);

        let base = self.arena.occupancy().base;
        self.make_room(&meta);
        let placed = self.place(id, located, provider, lease, wanted);
        self.events.record(match &placed {
            Ok(handle) => Event::placed(id, by, &meta, cost, handle.address - base),
            Err(why) => Event::refused(id, by, &meta, cost, *why),
        });
        placed
    }

    /// The provider that can produce an object, or `NoProvider`. Borrows
    /// the manager's lifetime, not `self`, so the table stays free.
    fn provider_for(&self, meta: &ObjectMeta) -> Result<&'a dyn Provider> {
        self.providers
            .get(meta.provider.0 as usize)
            .copied()
            .flatten()
            .ok_or(Error::NoProvider)
    }

    /// Places, fetches, and only then records residency: a resident entry
    /// whose bytes never arrived would read as a hit.
    fn place(
        &mut self,
        id: ObjectId,
        located: Located,
        provider: &dyn Provider,
        lease: Lease,
        wanted: NextUse,
    ) -> Result<Handle> {
        let size = located.size;
        let (address, into) = self.arena.place(size)?;
        provider.read(located, 0, into)?;
        self.counters.resident(i64::from(size));

        let now = self.clock;
        let placed = self.table.get_mut(id).ok_or(Error::BadObject)?;
        placed.resident_at = address;
        placed.next_use = wanted;
        placed.placed_tick = now;
        placed.used_tick = now;
        placed.tier = Tier::Warm;
        placed.share_count = u16::from(lease.pins());
        placed.reuse_count = placed.reuse_count.saturating_add(1);
        Ok(Handle { id, address, size })
    }
}
