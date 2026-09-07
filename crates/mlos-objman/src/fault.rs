//! What a model fault says.
//!
//! A page fault says *an address was not mapped*. That is all a page
//! table knows, and it is why paging has to guess: the only thing an
//! ordinary kernel can do with a fault is note the address and hope
//! recency predicts the future.
//!
//! A model fault says *which layer's weights, for which model, on behalf
//! of which session, wanted by when*. Everything in `docs/PRD.md` --
//! choosing a provider, choosing what to evict, deciding whether to
//! recompute instead of fetch -- is a decision this structure makes
//! possible and an address does not.

use mlos_abi::{Error, ObjectClass, ObjectId, Result};
use mlos_objtab::{CostNs, SessionId, Tier};
use mlos_provider::Located;

use mlos_objtab::ObjectMeta;
use mlos_provider::Provider;

use crate::{Handle, Lease, Manager};

/// A miss on the object table, described.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ModelFault {
    /// What kind of state was wanted. The axis every metric in
    /// `docs/PRD.md` s.5.2 is broken down by: "40,000 faults" is
    /// meaningless, "38,000 of them cold KV blocks" is a diagnosis.
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
    ///
    /// Carried in the fault rather than looked up later because it is
    /// what makes the fault comparable: a policy asked to free space
    /// needs to weigh this against what it would have to throw away.
    pub cost: CostNs,
}

impl ModelFault {
    /// Describes a miss on `id`.
    ///
    /// `None` if the id does not decode -- an all-zero word, or a class
    /// this ABI does not define. A fault on something that is not an
    /// object is a bug in the caller, not a residency problem.
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
    ///
    /// Order matters. The fault is described *before* anything is
    /// attempted, so a failure to find room still leaves a record of what
    /// was wanted -- the difference between a system that can explain why
    /// it refused and one that just refuses.
    pub(crate) fn service(&mut self, id: ObjectId, lease: Lease, by: SessionId) -> Result<Handle> {
        let meta = *self.table.get(id).ok_or(Error::BadObject)?;
        let provider = self.provider_for(&meta)?;
        let located = Located::new(id, &meta);

        let cost = provider.cost(located).for_bytes(meta.size);
        self.last_fault = Some(ModelFault::new(id, by, cost).ok_or(Error::BadClass)?);

        provider.prefetch(located)?;
        self.place(id, meta.size, lease)
    }

    /// The provider that can produce an object, or `NoProvider`.
    ///
    /// A distinct failure from `BadObject`: one means nobody registered
    /// the object, the other that nobody attached the thing that could
    /// fetch it. The reference borrows the *manager's* lifetime, not the
    /// borrow of `self` -- otherwise holding a provider would lock the
    /// table it is about to place into.
    fn provider_for(&self, meta: &ObjectMeta) -> Result<&'a dyn Provider> {
        self.providers
            .get(meta.provider.0 as usize)
            .copied()
            .flatten()
            .ok_or(Error::NoProvider)
    }

    /// Makes room and records where the object now lives.
    ///
    /// Separate because its failure means something different: no room is
    /// a residency decision nobody has made yet, and it is the error a
    /// session's admission contract exists to prevent.
    fn place(&mut self, id: ObjectId, size: u32, lease: Lease) -> Result<Handle> {
        let address = self.arena.place(size)?;
        let placed = self.table.get_mut(id).ok_or(Error::BadObject)?;
        placed.resident_at = address;
        placed.tier = Tier::Warm;
        placed.share_count = u16::from(lease.pins());
        placed.reuse_count = placed.reuse_count.saturating_add(1);
        Ok(Handle { id, address, size })
    }
}
