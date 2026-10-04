//! A budget admits what it can honour, refuses the rest by name, and says
//! how many like these it would take.

use mlos_admit::{Capacity, Refusal};
use mlos_session::{Contract, Sessions};

/// A small budget: 14 KiB unpromised after the working set, 2 KiB of KV
/// per token of context, a 128-acquire token.
const SMALL: Capacity = Capacity {
    bytes: 32 << 10,
    reserved: 18 << 10,
    kv_per_token: 2048,
    token: 128,
};

fn context(tokens: u32) -> Contract {
    Contract {
        context: tokens,
        ..Contract::NONE
    }
}

fn ceiling(acquires: u32) -> Contract {
    Contract {
        latency_ceiling: acquires,
        ..Contract::NONE
    }
}

#[test]
fn bytes_are_promised_once_and_a_contract_must_fit_its_own_ceiling() {
    let mut live = Sessions::<4>::EMPTY;
    SMALL.admit(&live, &context(4)).expect("8 KiB of 14");
    live.create(context(4)).expect("room");
    assert_eq!(
        SMALL.admit(&live, &context(4)),
        Err(Refusal::Bytes {
            need: 8192,
            free: 6144
        })
    );
    SMALL.admit(&live, &context(3)).expect("6 KiB of 6");
    let tight = Contract {
        resident_ceiling: 4096,
        ..context(4)
    };
    assert_eq!(
        SMALL.admit(&live, &tight),
        Err(Refusal::Ceiling {
            ceiling: 4096,
            need: 8192
        })
    );
}

#[test]
fn a_ceiling_must_be_a_quarter_token_and_cover_the_others_overdue() {
    let mut live = Sessions::<4>::EMPTY;
    assert_eq!(
        SMALL.admit(&live, &ceiling(16)),
        Err(Refusal::Latency {
            ceiling: 16,
            least: 32
        })
    );
    live.create(ceiling(200)).expect("alone, anything over 32");
    assert_eq!(
        SMALL.admit(&live, &ceiling(100)),
        Err(Refusal::Latency {
            ceiling: 100,
            least: 128
        }),
        "the newcomer would wait one token behind the first"
    );
    SMALL
        .admit(&live, &ceiling(300))
        .expect("both cover one token");
    live.create(ceiling(300)).expect("room");
    assert_eq!(
        SMALL.admit(&live, &ceiling(300)),
        Err(Refusal::Latency {
            ceiling: 200,
            least: 256
        }),
        "a third would break the first's promise, so the third is refused"
    );
    SMALL
        .admit(&live, &Contract::NONE)
        .expect("no ceiling, no wait");
}

#[test]
fn slots_run_out_and_none_admits_everything_else() {
    let mut live = Sessions::<2>::EMPTY;
    live.create(context(1)).expect("one");
    live.create(context(1)).expect("two");
    assert_eq!(SMALL.admit(&live, &Contract::NONE), Err(Refusal::Slots));
    let mut live = Sessions::<4>::EMPTY;
    live.create(context(1_000_000)).expect("room");
    Capacity::NONE
        .admit(&live, &context(1_000_000))
        .expect("nothing is tested");
    Capacity::NONE
        .admit(&live, &ceiling(1))
        .expect("nothing is tested");
}

#[test]
fn admits_is_the_live_count_until_a_context_is_declared() {
    let mut live = Sessions::<8>::EMPTY;
    live.create(Contract::NONE).expect("room");
    live.create(Contract::NONE).expect("room");
    assert_eq!(SMALL.admits(&live), 2, "what M4 measured");
    assert_eq!(Capacity::NONE.admits(&live), 2);
    live.create(context(4)).expect("room");
    live.create(context(1)).expect("room");
    // 14 KiB over the mean need of (8 + 2) / 4 KiB: 5.6 sessions like these.
    assert_eq!(SMALL.admits(&live), 5);
    assert_eq!(Capacity::NONE.admits(&live), 4);
}
