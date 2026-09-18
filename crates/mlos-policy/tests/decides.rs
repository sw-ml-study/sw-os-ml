//! What known-next-use decides, and why.
//!
//! Each test pins one of the three rules the policy is supposed to
//! follow, on a resident set small enough to check by hand. The
//! measurements in `mlos-workload` say whether the policy WINS; these say
//! whether it is doing what it was designed to do, which is a different
//! question and the one that catches a scoring bug.

use mlos_abi::{Fields, ObjectClass, ObjectId};
use mlos_objtab::{
    CostNs, Mutability, NextUse, ObjectMeta, Precision, ProviderId, SessionId, Tier,
};
use mlos_policy::{NEXT_USE, Policy, Residency};

/// A resident set built by hand.
struct Held(Vec<(ObjectId, ObjectMeta)>);

impl Residency for Held {
    fn len(&self) -> usize {
        self.0.len()
    }
    fn at(&self, index: usize) -> Option<(ObjectId, ObjectMeta)> {
        self.0.get(index).copied()
    }
    /// The stream has not moved, so a position IS a distance here.
    fn now(&self) -> u32 {
        0
    }
}

/// An id distinguishable by number.
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

/// An object with a given future and a given cost of getting it back.
fn held(next_use: NextUse, reload: u32) -> ObjectMeta {
    ObjectMeta {
        size: 1024,
        precision: Precision::Q4,
        tier: Tier::Warm,
        home: Tier::Warm,
        provider: ProviderId(2),
        handle: 0,
        resident_at: 1,
        next_use,
        reuse_count: 0,
        placed_tick: 0,
        used_tick: 0,
        reload_cost: CostNs(reload),
        recompute_cost: CostNs::IMPOSSIBLE,
        share_count: 0,
        mutability: Mutability::Immutable,
        owner: SessionId(0),
    }
}

/// Which of these the policy would throw away.
fn victim(objects: &[(u16, ObjectMeta)]) -> u16 {
    let set = Held(objects.iter().map(|(n, meta)| (id(*n), *meta)).collect());
    let wanting = held(NextUse::At(1), 1000);
    let chosen = NEXT_USE.victim(&set, &wanting).expect("a victim");
    chosen.fields().tensor
}

#[test]
fn the_furthest_away_goes_first() {
    // Belady's rule, with cost held equal so only distance can decide.
    let far = held(NextUse::At(900), 1000);
    let near = held(NextUse::At(3), 1000);
    assert_eq!(victim(&[(1, near), (2, far)]), 2);
    assert_eq!(victim(&[(1, far), (2, near)]), 1, "and not by position");
}

#[test]
fn what_nothing_has_declared_a_future_for_goes_before_anything_known() {
    // Obvious because nothing KNOWS about it, not because nothing will
    // want it -- which is why it is a decision with a reason rather than
    // a maximum, and why step 007's streams are what make it meaningful.
    let undeclared = held(NextUse::Never, 1000);
    let distant = held(NextUse::At(100_000), 1000);
    assert_eq!(victim(&[(1, distant), (2, undeclared)]), 2);
}

#[test]
fn the_cheaper_to_get_back_goes_first_at_equal_distance() {
    // Belady assumes every miss costs the same, which is true for pages
    // and false for everything MLOS holds. At the same distance the
    // dearer object is worth keeping.
    let cheap = held(NextUse::At(500), 100_000);
    let dear = held(NextUse::At(500), 4_000_000);
    assert_eq!(victim(&[(1, dear), (2, cheap)]), 2);
}

#[test]
fn distance_still_beats_cost_when_it_is_large_enough() {
    // The cost term modulates the decision; it must not obliterate it.
    // The first version scaled by a thousand, which truncated every
    // expensive object's score to zero and left the policy choosing
    // between ties by table position.
    let near_and_cheap = held(NextUse::At(2), 100_000);
    let far_and_dear = held(NextUse::At(20_000), 4_000_000);
    assert_eq!(victim(&[(1, near_and_cheap), (2, far_and_dear)]), 2);
}

#[test]
fn a_guess_is_worth_less_than_knowledge_of_the_same_size() {
    // `Probability` is a router's distribution, not a distance. An expert
    // wanted with probability p is expected about 1/p steps away -- and
    // then halved, so a hint must be twice as good as knowledge before it
    // wins. Here the two horizons are equal, so the KNOWN one is evicted
    // and the guess is kept.
    let odds = u16::MAX / 1000; // about one step in a thousand
    let guessed = held(NextUse::Probability(odds), 1000);
    let known = held(NextUse::At(1000), 1000);
    assert_eq!(
        victim(&[(1, guessed), (2, known)]),
        2,
        "the known-far object should go before the merely-unlikely one"
    );
}

#[test]
fn an_empty_set_yields_no_victim() {
    let set = Held(Vec::new());
    assert!(set.is_empty());
    assert!(NEXT_USE.victim(&set, &held(NextUse::Never, 1)).is_none());
}
