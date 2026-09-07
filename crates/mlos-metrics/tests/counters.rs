//! What the counters have to get right.
//!
//! Not that addition works -- that the numbers answer the questions
//! `docs/PRD.md` s.5.2 asks, and stay answerable at the edges where a
//! naive counter would lie.

use mlos_abi::ObjectClass;
use mlos_metrics::Counters;

#[test]
fn faults_are_counted_per_class_and_by_cost() {
    let mut counters = Counters::EMPTY;
    for _ in 0..1000 {
        counters.fault(ObjectClass::Scale, 64);
    }
    for _ in 0..10 {
        counters.fault(ObjectClass::Expert, 40 << 20);
    }

    let report = counters.report();
    assert_eq!(report.total_faults(), 1010);
    assert_eq!(
        report.worst(),
        Some((ObjectClass::Scale, 1000)),
        "by count, scales dominate"
    );
    assert!(
        report.fetched[ObjectClass::Expert.index()] > report.fetched[ObjectClass::Scale.index()],
        "but by bytes moved, ten experts cost far more than a thousand scales -- \
         which is why both numbers are kept"
    );
}

/// `Rm` with no model registered is unanswerable, not zero. Reporting
/// zero would read as "nothing is resident" rather than "nothing exists".
#[test]
fn residency_without_a_model_is_unanswerable() {
    let counters = Counters::EMPTY;
    assert_eq!(counters.report().residency_per_mille(), None);
}

/// The ratio the whole system optimises: a small fraction of a large
/// model resident is the good case, not a failure.
#[test]
fn residency_reports_the_fraction_that_matters() {
    let mut counters = Counters::EMPTY;
    counters.registered(1 << 30); // a gigabyte of weights
    counters.resident(3 << 20); // three megabytes of them in memory

    assert_eq!(
        counters.report().residency_per_mille(),
        Some(2),
        "3 MiB of 1 GiB"
    );
    assert_eq!(counters.report().resident, 3 << 20);
}

/// Eviction makes residency go down. A counter that only rose would make
/// the first eviction look like a leak.
#[test]
fn residency_falls_when_objects_leave() {
    let mut counters = Counters::EMPTY;
    counters.registered(1000);
    counters.resident(800);
    counters.resident(-300);

    assert_eq!(counters.report().resident, 500);
    assert_eq!(counters.report().residency_per_mille(), Some(500));
}

/// Counters saturate rather than wrap. A wrap would make a long run
/// report a smaller number than a short one -- quietly wrong, where a
/// stuck counter is visibly stuck.
#[test]
fn counters_saturate_rather_than_wrap() {
    let mut counters = Counters::EMPTY;
    // Three, not two: i64::MAX is 2^63 - 1, so two of them reach 2^64 - 2
    // and stop just short. The point is the ceiling, so go past it.
    for _ in 0..3 {
        counters.resident(i64::MAX);
    }
    assert_eq!(counters.report().resident, u64::MAX, "held at the ceiling");

    counters.resident(-1);
    assert_eq!(
        counters.report().resident,
        u64::MAX - 1,
        "and still moves back down"
    );
}

/// Nothing counted yet is a real answer: no worst class, no faults.
#[test]
fn an_empty_report_says_nothing_happened() {
    let report = Counters::EMPTY.report();
    assert_eq!(report.total_faults(), 0);
    assert_eq!(report.worst(), None);
}
