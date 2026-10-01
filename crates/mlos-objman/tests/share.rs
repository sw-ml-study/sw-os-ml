//! `share_count` is the number of leases holding an object, and a policy
//! input: a shared object outlives a private one under pressure.

use core::cell::Cell;

use mlos_abi::{Fields, ObjectClass, ObjectId};
use mlos_objman::{Arena, Lease, Manager};
use mlos_objtab::{
    CostNs, Mutability, NextUse, ObjectMeta, Precision, ProviderId, SessionId, Tier,
};
use mlos_policy::NEXT_USE;
use mlos_provider::{Cost, Located, Provider};

struct Stub(Cell<u32>);

impl Provider for Stub {
    fn id(&self) -> ProviderId {
        ProviderId(2)
    }
    fn read(&self, object: Located, _offset: u32, _into: &mut [u8]) -> mlos_abi::Result<u32> {
        self.0.set(self.0.get() + 1);
        Ok(object.size)
    }
    fn cost(&self, _object: Located) -> Cost {
        Cost {
            latency: CostNs(1_000),
            bytes_per_ms: 1_000_000,
        }
    }
}

fn weight(tensor: u16) -> ObjectId {
    ObjectId::new(
        ObjectClass::WeightTile,
        Fields {
            model: 1,
            layer: 0,
            tensor,
            tile: 0,
        },
    )
}

fn meta() -> ObjectMeta {
    ObjectMeta {
        size: 1024,
        precision: Precision::Q4,
        tier: Tier::Cold,
        home: Tier::Cold,
        provider: ProviderId(2),
        handle: 0,
        resident_at: 0,
        next_use: NextUse::Never,
        reuse_count: 0,
        placed_tick: 0,
        used_tick: 0,
        reload_cost: CostNs(3_000_000),
        recompute_cost: CostNs::IMPOSSIBLE,
        share_count: 0,
        mutability: Mutability::Immutable,
        owner: SessionId(0),
    }
}

fn manager<'a>(stub: &'a Stub, bytes: usize) -> Manager<'a, 64> {
    let mut m = Manager::<64>::new(Arena::new(Vec::leak(vec![0u8; bytes])));
    m.attach(stub).expect("slot 2");
    for tensor in 0..4 {
        m.register(weight(tensor), meta()).expect("room");
    }
    m
}

fn shares(m: &Manager<'_, 64>, tensor: u16) -> u16 {
    m.table
        .get(weight(tensor))
        .map_or(u16::MAX, |o| o.share_count)
}

#[test]
fn holds_count_and_guesses_do_not() {
    let stub = Stub(Cell::new(0));
    let mut m = manager(&stub, 64 << 10);
    m.acquire(weight(0), Lease::Pin, SessionId(1))
        .expect("placed");
    assert_eq!(shares(&m, 0), 1, "a pin counts");
    m.acquire(weight(0), Lease::Borrow, SessionId(2))
        .expect("hit");
    assert_eq!(shares(&m, 0), 2, "a borrow counts");
    m.acquire(weight(0), Lease::Speculative, SessionId(3))
        .expect("hit");
    assert_eq!(shares(&m, 0), 2, "a guess does not");
    m.release(weight(0), Lease::Borrow).expect("known");
    assert_eq!(shares(&m, 0), 1);
    m.release(weight(0), Lease::Pin).expect("known");
    assert_eq!(shares(&m, 0), 0);
    m.consume(weight(0), SessionId(4)).expect("hit");
    assert_eq!(shares(&m, 0), 0, "a streaming access is held and let go");
}

#[test]
fn eviction_voids_every_lease() {
    let stub = Stub(Cell::new(0));
    let mut m = manager(&stub, 64 << 10);
    m.acquire(weight(1), Lease::Pin, SessionId(1))
        .expect("placed");
    m.acquire(weight(1), Lease::Pin, SessionId(2)).expect("hit");
    m.evict(weight(1)).expect("resident");
    assert_eq!(shares(&m, 1), 0);
    m.release(weight(1), Lease::Pin)
        .expect("releasing a voided lease is not an error");
    assert_eq!(shares(&m, 1), 0, "and does not go negative");
}

#[test]
fn a_shared_object_outlives_a_private_one_under_pressure() {
    let stub = Stub(Cell::new(0));
    // Room for exactly two tiles, so the third acquire must evict one.
    let mut m = manager(&stub, 2 * 1024);
    m.policy = Some(&NEXT_USE);
    m.acquire(weight(0), Lease::Pin, SessionId(1))
        .expect("private");
    m.acquire(weight(1), Lease::Pin, SessionId(1))
        .expect("shared, once");
    m.acquire(weight(1), Lease::Pin, SessionId(2))
        .expect("shared, twice");
    assert_eq!((shares(&m, 0), shares(&m, 1)), (1, 2));

    m.acquire(weight(2), Lease::Pin, SessionId(3))
        .expect("needs room");

    let resident = |t| m.table.get(weight(t)).is_some_and(|o| o.resident_at != 0);
    assert!(!resident(0), "the object one session held went first");
    assert!(resident(1), "the object two sessions held stayed");
    assert!(resident(2));
}
