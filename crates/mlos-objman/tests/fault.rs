//! The fault path, end to end, on the host.
//!
//! No VM: the manager is a table, an arena and a dispatch, and every
//! property that matters -- that a hit does not fault, that a fault
//! carries which layer was wanted, that a full arena refuses rather than
//! overwriting -- is visible without one.

use core::cell::Cell;

use mlos_abi::{Error, Fields, ObjectClass, ObjectId};
use mlos_objman::{Arena, Handle, Lease, Manager};
use mlos_objtab::{
    CostNs, Mutability, NextUse, ObjectMeta, Precision, ProviderId, SessionId, Tier,
};
use mlos_provider::{Cost, Located, Provider};

/// A provider that records what it was asked for and hands back nothing.
///
/// The bytes are not the point here -- the DRAM provider's own tests
/// cover copying. What this checks is *that a fault reached a provider at
/// all*, and which object it named.
struct Recording {
    reads: Cell<u32>,
    prefetches: Cell<u32>,
}

impl Provider for Recording {
    fn id(&self) -> ProviderId {
        ProviderId(2)
    }
    fn read(&self, object: Located, _offset: u32, _into: &mut [u8]) -> mlos_abi::Result<u32> {
        self.reads.set(self.reads.get() + 1);
        Ok(object.size)
    }
    fn cost(&self, _object: Located) -> Cost {
        // NVMe-ish: milliseconds to first byte, then a gigabyte a second.
        Cost {
            latency: CostNs(3_000_000),
            bytes_per_ms: 1_000_000,
        }
    }
    fn prefetch(&self, _object: Located) -> mlos_abi::Result<()> {
        self.prefetches.set(self.prefetches.get() + 1);
        Ok(())
    }
}

fn tile(layer: u16, tensor: u16) -> ObjectId {
    ObjectId::new(
        ObjectClass::WeightTile,
        Fields {
            model: 7,
            layer,
            tensor,
            tile: 3,
        },
    )
}

/// Cold weights: they live somewhere else and cannot be recomputed.
fn cold(size: u32) -> ObjectMeta {
    ObjectMeta {
        size,
        precision: Precision::Q4,
        tier: Tier::Cold,
        provider: ProviderId(2),
        handle: 0x1000,
        resident_at: 0,
        next_use: NextUse::Never,
        reuse_count: 0,
        reload_cost: CostNs(3_000_000),
        recompute_cost: CostNs::IMPOSSIBLE,
        share_count: 0,
        mutability: Mutability::Immutable,
        owner: SessionId(0),
    }
}

/// A fault names the layer, not an address. This is the whole difference
/// from a page fault, and it is what every policy in docs/PRD.md needs.
#[test]
fn a_fault_says_what_was_wanted() {
    let provider = Recording {
        reads: Cell::new(0),
        prefetches: Cell::new(0),
    };
    let mut manager = Manager::<64>::new(Arena::new(0x4000_0000, 1 << 20));
    manager.attach(&provider).expect("slot 2 exists");
    manager.register(tile(17, 4), cold(2048)).expect("room");

    let handle = manager
        .acquire(tile(17, 4), Lease::Pin, SessionId(9))
        .expect("should fault in");

    let fault = manager.last_fault.expect("a fault was recorded");
    assert_eq!(fault.class, ObjectClass::WeightTile);
    assert_eq!(
        (fault.model, fault.layer, fault.tensor, fault.tile),
        (7, 17, 4, 3)
    );
    assert_eq!(fault.session, SessionId(9), "on whose behalf");
    assert!(
        fault.cost > CostNs(3_000_000),
        "latency plus transfer, not just latency"
    );
    assert_eq!(handle.size, 2048);
}

/// The fast path must not fault. A hit is a table lookup and a counter --
/// no provider, no arena, no policy.
#[test]
fn a_hit_does_not_reach_the_provider() {
    let provider = Recording {
        reads: Cell::new(0),
        prefetches: Cell::new(0),
    };
    let mut manager = Manager::<64>::new(Arena::new(0x4000_0000, 1 << 20));
    manager.attach(&provider).unwrap();
    manager.register(tile(3, 0), cold(1024)).unwrap();

    let first = manager
        .acquire(tile(3, 0), Lease::Pin, SessionId(1))
        .unwrap();
    assert_eq!(
        provider.prefetches.get(),
        1,
        "the miss reached the provider"
    );

    manager.last_fault = None;
    let second = manager
        .acquire(tile(3, 0), Lease::Borrow, SessionId(2))
        .unwrap();

    assert_eq!(provider.prefetches.get(), 1, "the hit did not");
    assert!(manager.last_fault.is_none(), "and did not record a fault");
    assert_eq!(first.address, second.address, "same object, same place");
}

/// Sharing is counted, because it is what parameter-major scheduling is
/// built on: four sessions on one layer should cause one fetch.
#[test]
fn concurrent_holders_are_counted_and_fetch_once() {
    let provider = Recording {
        reads: Cell::new(0),
        prefetches: Cell::new(0),
    };
    let mut manager = Manager::<64>::new(Arena::new(0x4000_0000, 1 << 20));
    manager.attach(&provider).unwrap();
    manager.register(tile(13, 0), cold(4096)).unwrap();

    let handles: Vec<Handle> = (1..=4)
        .map(|session| {
            manager
                .acquire(tile(13, 0), Lease::Pin, SessionId(session))
                .expect("acquired")
        })
        .collect();

    assert_eq!(provider.prefetches.get(), 1, "one fetch for four sessions");
    assert!(
        handles
            .windows(2)
            .all(|pair| pair[0].address == pair[1].address)
    );
    let meta = manager.table.get(tile(13, 0)).unwrap();
    assert_eq!(meta.share_count, 4, "all four are holding it");
    assert_eq!(meta.tier, Tier::Warm, "and it is resident now");
}

/// A full arena refuses. It is the error a session's admission contract
/// exists to prevent, and refusing is what MLOS does instead of
/// overwriting something nobody chose to lose.
#[test]
fn a_full_arena_refuses_rather_than_overwriting() {
    let provider = Recording {
        reads: Cell::new(0),
        prefetches: Cell::new(0),
    };
    let mut manager = Manager::<64>::new(Arena::new(0x4000_0000, 4096));
    manager.attach(&provider).unwrap();
    manager.register(tile(1, 0), cold(4096)).unwrap();
    manager.register(tile(2, 0), cold(4096)).unwrap();

    let first = manager
        .acquire(tile(1, 0), Lease::Pin, SessionId(1))
        .unwrap();
    let second = manager.acquire(tile(2, 0), Lease::Pin, SessionId(1));

    assert_eq!(second, Err(Error::NoBudget));
    assert!(
        manager.last_fault.is_some(),
        "the refusal still says what was wanted"
    );
    assert_eq!(manager.last_fault.unwrap().layer, 2);
    assert_eq!(
        manager.table.get(tile(1, 0)).unwrap().resident_at,
        first.address,
        "and the object already there was not disturbed"
    );
}

/// An object nobody registered, and one whose provider is not attached,
/// are different failures and say so.
#[test]
fn missing_objects_and_missing_providers_differ() {
    let mut manager = Manager::<64>::new(Arena::new(0x4000_0000, 1 << 20));
    assert_eq!(
        manager.acquire(tile(1, 0), Lease::Pin, SessionId(1)),
        Err(Error::BadObject),
        "never registered"
    );

    manager.register(tile(1, 0), cold(16)).unwrap();
    assert_eq!(
        manager.acquire(tile(1, 0), Lease::Pin, SessionId(1)),
        Err(Error::NoProvider),
        "registered, but nothing can produce it"
    );
}
