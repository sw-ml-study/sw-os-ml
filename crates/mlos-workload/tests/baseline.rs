//! The process-major merge is the order G4 was measured on, to the
//! integer. The generators used to write this order by hand; now a
//! scheduler produces it from per-session streams, and these numbers
//! from docs/g4-report.md are what prove nothing moved.

use mlos_policy::{LRU, NEXT_USE};
use mlos_sim::replay;
use mlos_trace::Trace;
use mlos_workload::{Context, Decode, MINICPM, Real, Shape};

#[test]
fn the_synthetic_trace_still_gives_the_g4_numbers() {
    let decode = Decode::of(4, 40);
    let held = decode.trace();
    assert_eq!(held.len(), 25_200);
    let trace = Trace {
        header: decode.header(),
        accesses: &held,
    };
    assert_eq!(replay(&trace, &decode, 128 << 10, &LRU).reads, 25_200);
    assert_eq!(replay(&trace, &decode, 128 << 10, &NEXT_USE).reads, 12_722);
    assert_eq!(replay(&trace, &decode, 192 << 10, &LRU).reads, 11_942);
    assert_eq!(replay(&trace, &decode, 192 << 10, &NEXT_USE).reads, 3_480);
}

#[test]
fn the_real_shape_trace_still_gives_the_g4_numbers() {
    let shape = Shape::parse(MINICPM, 2).expect("sidecar");
    let real = Real::of(&shape, 4, 40, Context::DECODE_ONLY);
    let held = real.trace();
    assert_eq!(held.len(), 54_150);
    let trace = Trace {
        header: real.header(),
        accesses: &held,
    };
    assert_eq!(replay(&trace, &real, 1536 << 20, &LRU).reads, 54_150);
    assert_eq!(replay(&trace, &real, 1536 << 20, &NEXT_USE).reads, 44_309);
}

#[test]
fn the_kernel_replay_trace_is_unchanged() {
    // What the model disk carries and the guest replays: 2 sessions x 16
    // rounds, 4,448 accesses, next-use 3,748 reads at 32 KiB.
    let decode = Decode::of(2, 16);
    let held = decode.trace();
    assert_eq!(held.len(), 4_448);
    let trace = Trace {
        header: decode.header(),
        accesses: &held,
    };
    assert_eq!(replay(&trace, &decode, 32 << 10, &NEXT_USE).reads, 3_748);
}
