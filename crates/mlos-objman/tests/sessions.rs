//! A session owns its objects, is held to its ceiling, and takes what it
//! owned with it when it ends.

use core::cell::Cell;

use mlos_abi::{Error, Fields, ObjectClass, ObjectId};
use mlos_objman::{Arena, Contract, Lease, Manager};
use mlos_objtab::{
    CostNs, Mutability, NextUse, ObjectMeta, Precision, ProviderId, SessionId, Tier,
};
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

fn meta(owner: SessionId, size: u32) -> ObjectMeta {
    ObjectMeta {
        size,
        precision: Precision::Fp16,
        tier: Tier::Warm,
        home: Tier::Warm,
        provider: ProviderId(2),
        handle: 0,
        resident_at: 0,
        next_use: NextUse::Never,
        reuse_count: 0,
        placed_tick: 0,
        used_tick: 0,
        reload_cost: CostNs(400_000),
        recompute_cost: CostNs::IMPOSSIBLE,
        share_count: 0,
        mutability: Mutability::Mutable,
        owner,
    }
}

fn kv(session: u16, position: u16) -> ObjectId {
    ObjectId::new(
        ObjectClass::KvBlock,
        Fields {
            model: session,
            layer: 0,
            tensor: position,
            tile: 0,
        },
    )
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

fn manager<'a>(stub: &'a Stub) -> Manager<'a, 64> {
    let mut manager = Manager::<64>::new(Arena::new(Vec::leak(vec![0u8; 64 << 10])));
    manager.attach(stub).expect("slot 2");
    manager
}

#[test]
fn ending_a_session_evicts_what_it_owned_and_nothing_else() {
    let stub = Stub(Cell::new(0));
    let mut m = manager(&stub);
    let (a, b) = (SessionId(1), SessionId(2));
    m.sessions.adopt(a, Contract::NONE).expect("a");
    m.sessions.adopt(b, Contract::NONE).expect("b");
    for who in [a, b] {
        for at in 0..3 {
            m.register(kv(who.0, at), meta(who, 1024)).expect("room");
            m.acquire(kv(who.0, at), Lease::Streaming, who)
                .expect("placed");
        }
    }
    m.register(weight(0), meta(SessionId(0), 1024))
        .expect("room");
    m.acquire(weight(0), Lease::Pin, a).expect("placed");
    let used_before = m.arena.occupancy().used;
    assert_eq!(m.sessions.get(a).map(|s| s.resident), Some(3 * 1024));

    assert_eq!(m.destroy_session(a), Ok(3));

    assert_eq!(m.arena.occupancy().used, used_before - 3 * 1024);
    assert!(m.table.get(kv(1, 0)).is_none(), "a's blocks are forgotten");
    assert!(
        m.table.get(kv(2, 0)).is_some_and(|o| o.resident_at != 0),
        "b's stay"
    );
    assert!(
        m.table.get(weight(0)).is_some_and(|o| o.resident_at != 0),
        "shared weights stay"
    );
    assert!(m.sessions.get(a).is_none());
    assert_eq!(m.destroy_session(a), Err(Error::BadObject));
}

#[test]
fn a_ceiling_refuses_the_acquire_that_would_pass_it_without_evicting() {
    let stub = Stub(Cell::new(0));
    let mut m = manager(&stub);
    let s = SessionId(1);
    let promised = Contract {
        resident_ceiling: 2048,
        ..Contract::NONE
    };
    m.sessions.adopt(s, promised).expect("s");
    for at in 0..3 {
        m.register(kv(1, at), meta(s, 1024)).expect("room");
    }
    m.acquire(kv(1, 0), Lease::Streaming, s).expect("first");
    m.acquire(kv(1, 1), Lease::Streaming, s)
        .expect("second, at the ceiling");
    assert_eq!(
        m.acquire(kv(1, 2), Lease::Streaming, s),
        Err(Error::Refused)
    );
    assert_eq!(m.evictions, 0, "a refused acquire chose no victim");
    assert_eq!(m.sessions.get(s).map(|x| x.resident), Some(2048));
    assert_eq!(stub.0.get(), 2, "the provider was not asked for the third");
    // A hit on something already held is not charged again.
    m.acquire(kv(1, 0), Lease::Streaming, s).expect("hit");
    assert_eq!(m.sessions.get(s).map(|x| x.resident), Some(2048));
}

#[test]
fn objects_of_an_unknown_session_are_served_and_uncounted() {
    let stub = Stub(Cell::new(0));
    let mut m = manager(&stub);
    m.register(kv(5, 0), meta(SessionId(5), 1024))
        .expect("room");
    m.acquire(kv(5, 0), Lease::Streaming, SessionId(5))
        .expect("served");
    assert!(m.sessions.get(SessionId(5)).is_none());
    assert_eq!(m.destroy_all_sessions(), 0);
}
