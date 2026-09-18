//! Does the simulator actually know the future it claims to?
use mlos_abi::{Fields, ObjectClass, ObjectId};
use mlos_objtab::SessionId;
use mlos_sim::Foresight;
use mlos_trace::Access;

fn id(n: u16) -> ObjectId {
    ObjectId::new(
        ObjectClass::WeightTile,
        Fields {
            model: 1,
            layer: 0,
            tensor: n,
            tile: 0,
        },
    )
}

fn trace(order: &[u16]) -> Vec<Access> {
    order
        .iter()
        .map(|n| Access {
            session: SessionId(1),
            object: id(*n),
        })
        .collect()
}

#[test]
fn the_next_occurrence_of_each_access_is_found() {
    //        index: 0  1  2  3  4
    //       object: A  B  A  C  B
    // next occurrence: 2  4  -  -  -
    let held = trace(&[1, 2, 1, 3, 2]);
    let seen = Foresight::read(&held);
    let after = |tick: u32| seen.after(tick);
    assert_eq!(after(1), Some(2), "A at 0 is next wanted at 2");
    assert_eq!(after(2), Some(4), "B at 1 is next wanted at 4");
    assert_eq!(after(3), None, "A at 2 is never wanted again");
    assert_eq!(after(4), None, "C at 3 is never wanted again");
    assert_eq!(after(5), None, "B at 4 is never wanted again");
}

#[test]
fn a_repeated_object_chains_forward() {
    let held = trace(&[1, 1, 1, 1]);
    let seen = Foresight::read(&held);
    assert_eq!(seen.after(1), Some(1));
    assert_eq!(seen.after(2), Some(2));
    assert_eq!(seen.after(3), Some(3));
    assert_eq!(seen.after(4), None);
}

/// What the replay writes into the table, which is what a policy reads.
///
/// Regression. `Foresight::after` answers in trace INDICES and the caller
/// counts one-based ticks; the first version compared them directly, so
/// an object wanted on the very NEXT access was marked `Never` and became
/// the most evictable thing in the table.
#[test]
fn the_next_access_is_the_least_evictable_not_the_most() {
    use mlos_objtab::NextUse;
    use mlos_sim::wanted_at;

    //  index: 0  1  2
    // object: A  B  A
    // A is acquired at tick 1 and wanted again at index 2, which is
    // position 3 in the one-based counting a policy compares against.
    let held = trace(&[1, 2, 1]);
    let seen = Foresight::read(&held);
    assert_eq!(wanted_at(&seen, 1), NextUse::At(3), "A comes round again");
    assert_eq!(wanted_at(&seen, 3), NextUse::Never, "and then never");
}
