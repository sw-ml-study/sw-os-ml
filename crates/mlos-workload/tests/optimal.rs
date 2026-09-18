//! How close to optimal is known-next-use, and does the cost term pay?
//!
//! Belady's MIN is provably optimal for a uniform-cost cache, so pure
//! distance is the ceiling on reads. MLOS's policy deliberately does not
//! maximise that: it divides distance by what recovery costs, because a
//! weight tile cannot be recomputed at any price while an activation is
//! cheaper to rebuild than to keep, and Belady's assumption that every
//! miss costs the same is true for pages and false here.
//!
//! So this measures the trade directly: the cost term should give up a
//! very small number of reads and buy a real reduction in total recovery
//! cost. If it ever gives up reads AND costs more, it is not paying for
//! itself and should go.
//!
//! **It is also the bug detector.** A policy with strictly more
//! information than LRU cannot honestly lose to it, and a policy with a
//! cost term cannot honestly lose to pure Belady on cost. Both went wrong
//! at first -- see the crate docs for `mlos-sim`'s `foresight` -- and both
//! failures were invisible in every other test.

use mlos_abi::ObjectId;
use mlos_objtab::{NextUse, ObjectMeta};
use mlos_policy::{FIFO, LRU, NEXT_USE, Policy, Residency};
use mlos_sim::compare;
use mlos_trace::Trace;
use mlos_workload::Decode;

/// Belady's MIN: furthest away wins, cost ignored. The read ceiling.
struct Belady;

impl Policy for Belady {
    fn name(&self) -> &'static str {
        "belady"
    }

    fn victim(&self, resident: &dyn Residency, _wanting: &ObjectMeta) -> Option<ObjectId> {
        let mut best: Option<(ObjectId, u64)> = None;
        for index in 0..resident.len() {
            let Some((id, meta)) = resident.at(index) else {
                continue;
            };
            let horizon = match meta.next_use {
                NextUse::Never => u64::MAX,
                NextUse::At(position) => u64::from(position.saturating_sub(resident.now())),
                NextUse::Probability(_) => u64::MAX / 2,
            };
            if best.is_none_or(|(_, held)| horizon > held) {
                best = Some((id, horizon));
            }
        }
        best.map(|(id, _)| id)
    }
}

/// Budgets inside the band where policy can matter at all.
const BAND: [u64; 4] = [128, 160, 192, 224];

#[test]
fn known_next_use_beats_every_baseline_everywhere_in_the_band() {
    // The claim the milestone rests on. A budget where it loses is a bug
    // in the policy, not a finding about the thesis -- it has strictly
    // more information than either baseline.
    let decode = Decode::of(4, 40);
    let held = decode.trace();
    let trace = Trace {
        header: decode.header(),
        accesses: &held,
    };
    for kib in BAND {
        let policies = [&FIFO as &dyn Policy, &LRU, &NEXT_USE];
        let table = compare(&trace, &decode, kib * 1024, &policies);
        let of = |n: &str| table.iter().find(|(m, _)| *m == n).expect(n).1;
        let (fifo, lru, next) = (of("fifo").reads, of("lru").reads, of("next-use").reads);
        let gain = 100 - (next as i64 * 100 / fifo.min(lru) as i64);
        println!("{kib:>4} KiB: fifo {fifo}, lru {lru}, next-use {next} ({gain:+}%)");
        assert!(
            next < fifo.min(lru),
            "next-use lost at {kib} KiB: {next} against {}",
            fifo.min(lru)
        );
    }
}

#[test]
fn the_cost_term_pays_for_itself() {
    // Slightly more reads than optimal, and less total recovery cost. If
    // both went the wrong way the term would be buying nothing.
    let decode = Decode::of(4, 40);
    let held = decode.trace();
    let trace = Trace {
        header: decode.header(),
        accesses: &held,
    };
    for kib in BAND {
        let table = compare(
            &trace,
            &decode,
            kib * 1024,
            &[&Belady as &dyn Policy, &NEXT_USE],
        );
        let of = |n: &str| table.iter().find(|(m, _)| *m == n).expect(n).1;
        let (optimal, ours) = (of("belady"), of("next-use"));
        println!(
            "{kib:>4} KiB: belady {} reads / {} ms, next-use {} reads / {} ms",
            optimal.reads,
            optimal.cost / 1_000_000,
            ours.reads,
            ours.cost / 1_000_000
        );
        // Within a few per cent of the read ceiling.
        assert!(
            ours.reads * 100 < optimal.reads * 110,
            "{kib} KiB: {} reads against an optimum of {}",
            ours.reads,
            optimal.reads
        );
        assert!(
            ours.cost <= optimal.cost,
            "{kib} KiB: the cost term cost more"
        );
    }
}
