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

use mlos_objtab::SessionId;
use mlos_policy::{Demand, FIFO, LRU, NEXT_USE, Policy};
use mlos_sched::{Lane, ParameterMajor, merge};
use mlos_sim::{Outcome, replay};
use mlos_trace::Trace;
use mlos_workload::Decode;

/// The binary cargo built for this test.
const MLOS: &str = env!("CARGO_BIN_EXE_mlos");

/// Arena bytes both sides are given, in kibibytes. One number for every
/// policy and schedule; smaller than the model on purpose.
const BUDGET_KIB: u64 = 32;

/// How long to let the guest run: eight replays under TCG, with room.
const SECONDS: &str = "240";

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
    let console = guest();
    for (name, policy) in POLICIES {
        for how in SCHEDULES {
            let label = format!("{name} {how}");
            let want = expected(&decode, policy, how);
            assert_eq!(
                said(&console, &label).as_deref(),
                Some(counts(&label, &want).as_str()),
                "{label}: the kernel and the simulator disagree.\n\n{console}"
            );
        }
    }
}

/// Boots aarch64 under TCG and runs the script.
fn guest() -> String {
    // `--arch aarch64` explicitly: without it `run` means the host's
    // architecture, which on a Linux PC is x86-64 (tests/boot_x86.rs).
    let out = Command::new(MLOS)
        .args(["--arch", "aarch64"])
        .args(["run", "tcg", "--capture", SECONDS, "--run"])
        .arg(script())
        .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
        .output()
        .expect("mlos runs");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// The schedules, by the name the shell knows them by.
const SCHEDULES: [&str; 2] = ["process", "parameter"];

/// What the simulator says `policy` under `how` should count: the
/// process-major order is the trace itself; the parameter-major one is
/// the same lanes merged by the same scheduler crate the kernel links.
fn expected(decode: &Decode, policy: &dyn Policy, how: &str) -> Outcome {
    let lanes: Vec<Lane> = (0..decode.sessions)
        .map(|s| Lane {
            session: SessionId(s + 1),
            ceiling: 0,
            tokens: decode.tokens(s),
        })
        .collect();
    let held = match how {
        "parameter" => merge(&mut ParameterMajor, &lanes),
        _ => decode.trace(),
    };
    let trace = Trace {
        header: decode.header(),
        accesses: &held,
    };
    replay(&trace, decode, BUDGET_KIB * 1024, policy)
}

/// The shell script: every policy under every schedule, each on a fresh
/// model, because a replay that started with the last one's residents
/// would measure the handover rather than the policy.
fn script() -> String {
    POLICIES
        .iter()
        .flat_map(|(name, _)| {
            SCHEDULES.map(|how| format!("model {BUDGET_KIB};replay {name} {how}"))
        })
        .collect::<Vec<_>>()
        .join(";")
}

/// What the shell printed for one policy and schedule, trimmed.
fn said(console: &str, label: &str) -> Option<String> {
    console
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with(&format!("{label}:")))
        .map(str::to_owned)
}

/// The same line, built from what the simulator counted.
fn counts(label: &str, out: &Outcome) -> String {
    let Outcome {
        reads,
        hits,
        bytes,
        evicted,
        refused,
        ..
    } = out;
    format!(
        "{label}: {reads} reads, {hits} hits, {bytes} bytes, {evicted} evicted, {refused} refused, {}",
        out.headline(BUDGET_KIB * 1024)
    )
}
