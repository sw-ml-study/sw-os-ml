//! The kernel and the simulator, on the same trace, under the same budget.
//!
//! The claim M3 step 009 exists to make: `mlos-sim`'s numbers describe
//! something that actually ran. Both sides link the same `mlos-policy`
//! crate, read the same trace, and are given the same budget, so their
//! counts are the same integers arrived at by the same decisions.
//!
//! **Exactly, not approximately.** Reads, hits, bytes, evictions and
//! refusals are decided one access at a time. A disagreement of one means
//! one of the two is wrong somewhere, and the one that was wrong has been
//! the kernel every time so far:
//!
//! - `acquire` called an object resident when its TIER said `Warm`,
//!   rather than when it had an address. Weight tiles start `Cold` and
//!   agreed; KV blocks start `Warm` and a never-fetched one read as a hit
//!   at address zero.
//! - `Stream` was cyclic -- one declared period, cursor running past the
//!   end -- which a KV cache is not, because it accumulates. Every block
//!   read as `Never`, the most evictable thing in the table, and next-use
//!   spent 27 reads throwing away exactly what it was about to want.
//!
//! Neither would have been found by a test of either side alone. Both
//! sides were self-consistent and only the comparison disagreed.
//!
//! Under TCG rather than HVF, for the reason `boot.rs` gives: a number
//! that came out differently on someone else's machine would be worse
//! than no number.

use std::process::Command;

use mlos_policy::{Demand, FIFO, LRU, NEXT_USE, Policy};
use mlos_sim::{Outcome, replay};
use mlos_trace::Trace;
use mlos_workload::Decode;

/// The binary cargo built for this test.
const MLOS: &str = env!("CARGO_BIN_EXE_mlos");

/// Arena bytes both sides are given, in kibibytes.
///
/// One number for every policy, which is the enforcement: a comparison
/// where the policies had different budgets measures nothing. Smaller
/// than the model on purpose -- a run where everything fits makes every
/// policy identical.
const BUDGET_KIB: u64 = 32;

/// How long to let the guest run.
///
/// Four replays of several thousand accesses each, under an emulator that
/// translates every instruction. Generous, because a flaky timeout is
/// worse than a slow test.
const SECONDS: &str = "120";

/// The policies, by the name the shell knows them by.
const POLICIES: [(&str, &dyn Policy); 4] = [
    ("demand", &Demand),
    ("fifo", &FIFO),
    ("lru", &LRU),
    ("next-use", &NEXT_USE),
];

#[test]
#[ignore = "boots a VM; run with --ignored"]
fn the_kernel_counts_what_the_simulator_counted() {
    let (sessions, rounds) = mlos_image_map::runtime::REPLAY;
    let decode = Decode::of(sessions, rounds);
    let held = decode.trace();
    let trace = Trace {
        header: decode.header(),
        accesses: &held,
    };

    let console = guest();
    for (name, policy) in POLICIES {
        let want = replay(&trace, &decode, BUDGET_KIB * 1024, policy);
        let found = said(&console, name);
        assert_eq!(
            found.as_deref(),
            Some(counts(name, &want).as_str()),
            "{name}: the kernel and the simulator disagree.\n\n{console}"
        );
    }
}

/// Boots, registers the model afresh before each policy, and replays.
///
/// Afresh because `model` rebuilds the manager: a replay that started
/// with the previous policy's residents would be measuring the handover
/// rather than the policy.
fn guest() -> String {
    let script = POLICIES
        .map(|(name, _)| format!("model {BUDGET_KIB};replay {name}"))
        .join(";");
    let out = Command::new(MLOS)
        .args(["run", "tcg", "--capture", SECONDS, "--run"])
        .arg(script)
        .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
        .output()
        .expect("mlos runs");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// What the shell printed for one policy, without its leading spaces.
fn said(console: &str, name: &str) -> Option<String> {
    console
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with(&format!("{name}:")))
        .map(str::to_owned)
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
