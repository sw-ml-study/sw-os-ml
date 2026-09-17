//! The model object manager.
//!
//! Where a conventional kernel has a VM subsystem, MLOS has this. It owns
//! the object table, the arena resident objects live in, and the fault
//! path between them.
//!
//! The fast path is the design constraint. A hit must be a table lookup
//! and nothing else: no allocation, no call out to a policy, no lock. A
//! fault that costs an IPC round trip before it even knows where to look
//! is a fault too expensive to have, which is why the *table* lives in
//! the kernel while *policy* does not (`docs/architecture.md` s.4).

#![no_std]
#![forbid(unsafe_code)]

mod arena;
mod fault;
mod lease;

use mlos_abi::{Error, ObjectId, Result};
use mlos_events::{Event, Ring};
use mlos_metrics::Counters;
use mlos_objtab::{ObjectMeta, ProviderId, SessionId, Table, Tier};
use mlos_provider::Provider;

pub use arena::Arena;
pub use fault::ModelFault;
pub use lease::{Handle, Lease};

/// How many providers can be attached.
pub const MAX_PROVIDERS: usize = 8;

/// The object manager.
pub struct Manager<'a, const N: usize> {
    /// What is known about each object.
    pub table: Table<N>,
    /// Where resident objects live. Public because how full it is, and
    /// how much would still fit, is a question anything may ask.
    pub arena: Arena,
    providers: [Option<&'a dyn Provider>; MAX_PROVIDERS],
    /// The most recent fault, for a caller to report on.
    pub last_fault: Option<ModelFault>,
    /// What has happened, in order.
    ///
    /// The field is `events` and the crate is `mlos-events`; the shell
    /// verb that prints them is still `trace`, because that is what
    /// someone types and it is still what it does. The name `mlos-trace`
    /// now belongs to the M3 access trace, which is a different thing:
    /// what the workload ASKED FOR, rather than what this did about it.
    ///
    /// Counters say how much; this says what, and when relative to
    /// everything else. A viewer animating residency needs the sequence,
    /// not the totals -- and the totals can be rebuilt from the sequence
    /// where the reverse is not true.
    pub events: Ring,
    /// A monotonic count of acquires, for `ObjectMeta`'s two ticks.
    ///
    /// The kernel keeps them even though nothing in the kernel reads them
    /// yet: a field the simulator fills and the kernel does not is a
    /// field the two disagree about, and step 009 replays the same trace
    /// in both and requires the counts to match exactly.
    pub clock: u32,
    /// What has happened, counted.
    ///
    /// Kept here rather than by a caller because this is where the events
    /// are: a fault that the manager serviced and a caller forgot to
    /// count is a fault that did not happen, as far as any measurement is
    /// concerned.
    pub counters: Counters,
}

impl<'a, const N: usize> Manager<'a, N> {
    /// A manager over `arena`.
    pub fn new(arena: Arena) -> Self {
        Self {
            table: Table::EMPTY,
            arena,
            providers: [None; MAX_PROVIDERS],
            last_fault: None,
            clock: 0,
            events: Ring::EMPTY,
            counters: Counters::EMPTY,
        }
    }

    /// Makes a provider available to serve faults.
    pub fn attach(&mut self, provider: &'a dyn Provider) -> Result<()> {
        let slot = self
            .providers
            .get_mut(provider.id().0 as usize)
            .ok_or(Error::NoProvider)?;
        *slot = Some(provider);
        Ok(())
    }

    /// Gets an object, faulting it in if it is not resident.
    ///
    /// The fast path -- a resident object -- is a table lookup, a tier
    /// comparison and a counter. Everything else is [`Self::service`].
    pub fn acquire(&mut self, id: ObjectId, lease: Lease, by: SessionId) -> Result<Handle> {
        self.clock = self.clock.saturating_add(1);
        if let Some(meta) = self.table.get(id)
            && meta.tier <= Tier::Warm
        {
            let (address, size) = (meta.resident_at, meta.size);
            let now = self.clock;
            let claim = self.table.get_mut(id).ok_or(Error::BadObject)?;
            claim.share_count = claim.share_count.saturating_add(1);
            claim.reuse_count = claim.reuse_count.saturating_add(1);
            claim.used_tick = now;
            self.events.record(Event::hit(id, by, size));
            return Ok(Handle { id, address, size });
        }
        self.service(id, lease, by)
    }

    /// Registers an object the manager may later be asked for.
    pub fn register(&mut self, id: ObjectId, meta: ObjectMeta) -> Result<()> {
        if !self.table.insert(id, meta) {
            return Err(Error::NoBudget);
        }
        self.counters.registered(meta.size);
        Ok(())
    }
}

/// The provider slot the DRAM tier conventionally occupies.
pub const RESIDENT: ProviderId = ProviderId(1);
