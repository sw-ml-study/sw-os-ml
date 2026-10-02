//! `Ps`, `Ks` and `Ss` from counts, and from a report.

use mlos_abi::ObjectClass;
use mlos_metrics::{Counters, Headline};

#[test]
fn one_use_per_read_is_a_thousand_and_four_sessions_sharing_is_four_thousand() {
    assert_eq!(Headline::of(1000, 1000, 0, 1, 1).ps_per_mille, 1000);
    assert_eq!(Headline::of(4000, 1000, 0, 4, 1).ps_per_mille, 4000);
    assert_eq!(
        Headline::of(0, 0, 0, 0, 0),
        Headline::default(),
        "zeros answer zero"
    );
}

#[test]
fn ks_is_kv_per_session_and_ss_is_sessions_per_gib() {
    let h = Headline::of(0, 0, 4 << 20, 4, 1536 << 20);
    assert_eq!(h.ks, 1 << 20);
    assert_eq!(h.ss_milli, 2666, "four sessions in a GiB and a half");
    assert_eq!(
        Headline::of(0, 0, 0, 2, 32 << 10).ss_milli,
        65_536_000,
        "the kernel's 32 KiB arena"
    );
}

#[test]
fn a_report_applies_hits_and_reads_and_counts_only_weights_for_ps() {
    let mut c = Counters::EMPTY;
    c.fault(ObjectClass::WeightTile, 1024);
    c.hit(ObjectClass::WeightTile, 1024);
    c.hit(ObjectClass::WeightTile, 1024);
    c.fault(ObjectClass::KvBlock, 256);
    c.hit(ObjectClass::KvBlock, 256);
    c.held(ObjectClass::KvBlock, 256);
    c.held(ObjectClass::WeightTile, 1024);
    let h = c.report().headline(2, 1 << 30);
    assert_eq!(h.ps_per_mille, 3000, "one read served three uses");
    assert_eq!(h.ks, 128, "256 B of KV over two sessions");
    assert_eq!(h.ss_milli, 2000);
    assert_eq!(c.report().resident, 1280, "resident is the sum of held");
}
