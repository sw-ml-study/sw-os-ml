//! One kernel, two architectures, one table (saga `mlos-x86-64`, step
//! `x86-replay`).
//!
//! `replay.rs` proves the aarch64 guest counts what the simulator counts.
//! This proves the x86-64 guest does too, AND that it prints the very
//! same lines as the aarch64 guest -- both booted here, side by side, on
//! the same trace, the same budget, the same four policies. Every crate
//! above the HAL is shared; this is where "architecture-neutral" is
//! checked rather than claimed.
//!
//! Exactly, not approximately, for the reason `replay.rs` gives: the
//! counts are decided one access at a time, and a disagreement of one is
//! a bug on one side. Under TCG on both architectures.
//!
//! Its own file, so the two lanes never edit the same test.

use std::{process::Command, thread};

use mlos_policy::{Demand, FIFO, LRU, NEXT_USE, Policy};
use mlos_sim::{Outcome, replay};
use mlos_trace::Trace;
use mlos_workload::Decode;

/// The binary cargo built for this test.
const MLOS: &str = env!("CARGO_BIN_EXE_mlos");

/// The same budget `replay.rs` gives both of its sides.
const BUDGET_KIB: u64 = 32;

/// How long each guest runs: four replays under TCG, with room.
const SECONDS: &str = "150";

/// The policies, by the name the shell knows them by.
const POLICIES: [(&str, &dyn Policy); 4] = [
    ("demand", &Demand),
    ("fifo", &FIFO),
    ("lru", &LRU),
    ("next-use", &NEXT_USE),
];

#[test]
#[ignore = "boots two VMs; run with --ignored"]
fn x86_64_counts_what_aarch64_and_the_simulator_counted() {
    if missing("qemu-system-x86_64") || missing("qemu-system-aarch64") {
        return;
    }
    let x86 = thread::spawn(|| guest("x86-64"));
    let arm = guest("aarch64");
    let x86 = x86.join().expect("x86-64 guest");
    let (sessions, rounds) = mlos_image_map::runtime::REPLAY;
    let decode = Decode::of(sessions, rounds);
    let held = decode.trace();
    let trace = Trace {
        header: decode.header(),
        accesses: &held,
    };
    for (name, policy) in POLICIES {
        let want = counts(name, &replay(&trace, &decode, BUDGET_KIB * 1024, policy));
        let (x, a) = (said(&x86, name), said(&arm, name));
        assert_eq!(
            x.as_deref(),
            Some(want.as_str()),
            "{name}: x86-64 vs the simulator\n\n{x86}"
        );
        assert_eq!(
            x, a,
            "{name}: x86-64 vs aarch64\n\nx86-64:\n{x86}\n\naarch64:\n{arm}"
        );
    }
}

/// True, with the reason printed, if `qemu` is not installed: this
/// comparison needs both guests, and says by name which one it cannot run.
fn missing(qemu: &str) -> bool {
    let found = Command::new(qemu)
        .arg("--version")
        .output()
        .is_ok_and(|out| out.status.success());
    if !found {
        eprintln!("skipped: {qemu} is not installed (see `mlos doctor`)");
    }
    !found
}

/// Boots `arch` under TCG and replays every policy, each on a fresh model.
fn guest(arch: &str) -> String {
    let script = POLICIES
        .map(|(name, _)| format!("model {BUDGET_KIB};replay {name}"))
        .join(";");
    let out = Command::new(MLOS)
        .args([
            "--arch",
            arch,
            "run",
            "tcg",
            "--capture",
            SECONDS,
            "--run",
            &script,
        ])
        .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
        .output()
        .expect("mlos runs");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// What the shell printed for one policy, without its leading spaces.
fn said(console: &str, name: &str) -> Option<String> {
    let line = console
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with(&format!("{name}:")));
    line.map(str::to_owned)
}

/// The same line, built from what the simulator counted.
fn counts(name: &str, out: &Outcome) -> String {
    let Outcome {
        reads,
        hits,
        bytes,
        evicted,
        refused,
        ..
    } = out;
    format!(
        "{name}: {reads} reads, {hits} hits, {bytes} bytes, {evicted} evicted, {refused} refused"
    )
}
