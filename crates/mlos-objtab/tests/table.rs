//! The object table, exercised on the host.
//!
//! No VM and no kernel: it is a data structure, and the properties that
//! matter -- that a removal does not hide its neighbours, that a dense
//! layer sweep does not pile into one bucket -- are exactly the ones a
//! boot test would never notice.

use mlos_abi::{Fields, ObjectClass, ObjectId};
use mlos_objtab::{
    CostNs, Mutability, NextUse, ObjectMeta, Precision, ProviderId, SessionId, Table,
};

/// A weight tile of the given layer and tensor.
fn tile(layer: u16, tensor: u16) -> ObjectId {
    ObjectId::new(
        ObjectClass::WeightTile,
        Fields {
            model: 1,
            layer,
            tensor,
            tile: 0,
        },
    )
}

/// Immutable weights: no recompute, one copy for everyone.
fn weights(size: u32) -> ObjectMeta {
    ObjectMeta {
        size,
        precision: Precision::Q4,
        tier: mlos_objtab::Tier::Cold,
        provider: ProviderId(1),
        next_use: NextUse::Never,
        reuse_count: 0,
        reload_cost: CostNs(3_000_000),
        recompute_cost: CostNs::IMPOSSIBLE,
        share_count: 0,
        mutability: Mutability::Immutable,
        owner: SessionId(0),
    }
}

#[test]
fn an_object_can_be_recorded_and_found() {
    let mut table = Table::<64>::default();
    assert!(table.insert(tile(3, 0), weights(2048)));

    assert_eq!(table.get(tile(3, 0)).map(|meta| meta.size), Some(2048));
    assert_eq!(
        table.get(tile(4, 0)),
        None,
        "a neighbour is not the same object"
    );
}

/// Re-registering replaces rather than duplicating: a model reloaded at a
/// different precision is the same object in a different form.
#[test]
fn reinserting_replaces() {
    let mut table = Table::<64>::default();
    table.insert(tile(3, 0), weights(2048));
    let mut smaller = weights(1024);
    smaller.precision = Precision::Q3;
    table.insert(tile(3, 0), smaller);

    let found = table.get(tile(3, 0)).expect("still there");
    assert_eq!(found.size, 1024);
    assert_eq!(found.precision, Precision::Q3);
}

/// Residency changes are the point of `get_mut`: the table is what
/// records that an object moved tier or gained a lease.
#[test]
fn residency_can_be_updated_in_place() {
    let mut table = Table::<64>::default();
    table.insert(tile(3, 0), weights(2048));

    let meta = table.get_mut(tile(3, 0)).expect("present");
    meta.tier = mlos_objtab::Tier::Warm;
    meta.share_count += 1;
    meta.next_use = NextUse::Distance(32);

    let found = table.get(tile(3, 0)).expect("present");
    assert_eq!(found.tier, mlos_objtab::Tier::Warm);
    assert_eq!(found.share_count, 1);
    assert_eq!(found.next_use, NextUse::Distance(32));
}

/// The property linear probing gets wrong if removal writes a vacancy:
/// deleting one object must not hide another that probed past its slot.
#[test]
fn removing_does_not_hide_its_neighbours() {
    let mut table = Table::<16>::default();
    let ids: [ObjectId; 12] = core::array::from_fn(|n| tile(n as u16, 0));
    for id in ids {
        assert!(table.insert(id, weights(1)), "should fit");
    }

    for id in ids.iter().step_by(2) {
        assert!(table.remove(*id));
    }
    for id in ids.iter().skip(1).step_by(2) {
        assert!(table.get(*id).is_some(), "survivor {id:?} went missing");
    }
    for id in ids.iter().step_by(2) {
        assert!(table.get(*id).is_none(), "removed {id:?} came back");
    }
}

/// A full table refuses rather than evicting something nobody asked to
/// lose. Losing a registration is bad; losing a different one silently is
/// worse.
#[test]
fn a_full_table_refuses() {
    let mut table = Table::<8>::default();
    for layer in 0..8 {
        assert!(table.insert(tile(layer, 0), weights(1)));
    }
    assert!(!table.insert(tile(99, 0), weights(1)), "no room");
    assert!(table.get(tile(0, 0)).is_some(), "and nothing was displaced");
}

/// A dense layer sweep walks consecutive ids. Masking the raw value would
/// pile a whole layer into adjacent slots; the hash has to scatter them,
/// or the fast path degenerates into a linear scan exactly when a
/// transformer is running.
#[test]
fn a_dense_sweep_does_not_pile_into_one_bucket() {
    let mut table = Table::<128>::default();
    for tensor in 0..64 {
        assert!(
            table.insert(tile(7, tensor), weights(1)),
            "sweep should fit"
        );
    }
    for tensor in 0..64 {
        assert!(table.get(tile(7, tensor)).is_some(), "tensor {tensor} lost");
    }
    assert!(table.get(tile(7, 64)).is_none());
}
