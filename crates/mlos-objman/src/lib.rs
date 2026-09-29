//! The model object manager: the object table, the arena, and the fault
//! path between them.
//!
//! Invariant: a hit is a table lookup and nothing else, and residency is
//! `resident_at != 0`, never the tier. Design and history:
//! docs/notes/mlos-objman.md.

#![no_std]
#![forbid(unsafe_code)]

pub(crate) mod evict;
mod fault;
mod lease;

use mlos_abi::{Error, ObjectId, Result};
use mlos_events::{Event, Ring};
use mlos_metrics::Counters;
use mlos_objtab::{ObjectMeta, ProviderId, SessionId, Table};
use mlos_policy::Policy;
use mlos_provider::Provider;
use mlos_stream::Stream;

pub use fault::ModelFault;
pub use lease::{Handle, Lease};
pub use mlos_arena::{Arena, Occupancy};

/// How many providers can be attached.
pub const MAX_PROVIDERS: usize = 8;

/// The object manager.
pub struct Manager<'a, const N: usize> {
    /// What is known about each object.
    pub table: Table<N>,
    /// Where resident objects live. Public so anything may ask how full it
    /// is.
    pub arena: Arena<'a>,
    providers: [Option<&'a dyn Provider>; MAX_PROVIDERS],
    /// The most recent fault, for a caller to report on.
    pub last_fault: Option<ModelFault>,
    /// What has happened, in order.
    pub events: Ring,
    /// What decides a victim when the arena is full. `None` is demand
    /// paging: a full arena refuses rather than choosing.
    pub policy: Option<&'a dyn Policy>,
    /// What this session has declared it will acquire, and where it is.
    /// Empty answers `Never` for everything.
    pub stream: Stream<'a>,
    /// How many objects have been thrown out. Counted, not derived: the
    /// event ring overflows on a long run.
    pub evictions: u64,
    /// A monotonic count of acquires, for `ObjectMeta`'s two ticks.
    pub clock: u32,
    /// What has happened, counted. Kept here because this is where the
    /// events are.
    pub counters: Counters,
}

impl<'a, const N: usize> Manager<'a, N> {
    /// A manager over `arena`.
    pub fn new(arena: Arena<'a>) -> Self {
        Self {
            table: Table::EMPTY,
            arena,
            providers: [None; MAX_PROVIDERS],
            last_fault: None,
            evictions: 0,
            policy: None,
            stream: Stream::EMPTY,
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

    /// Gets an object, faulting it in if it is not resident. A resident
    /// object is `resident_at != 0`, not a tier. The stream is asked once
    /// here and the answer written into whichever path serves the acquire.
    pub fn acquire(&mut self, id: ObjectId, lease: Lease, by: SessionId) -> Result<Handle> {
        self.clock = self.clock.saturating_add(1);
        let wanted = self.stream.next_after(id);
        if let Some(meta) = self.table.get(id)
            && meta.resident_at != 0
        {
            let (address, size) = (meta.resident_at, meta.size);
            let now = self.clock;
            let claim = self.table.get_mut(id).ok_or(Error::BadObject)?;
            claim.share_count = claim.share_count.saturating_add(1);
            claim.reuse_count = claim.reuse_count.saturating_add(1);
            claim.used_tick = now;
            claim.next_use = wanted;
            self.events.record(Event::hit(id, by, size));
            return Ok(Handle { id, address, size });
        }
        self.service(id, lease, by, wanted)
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
