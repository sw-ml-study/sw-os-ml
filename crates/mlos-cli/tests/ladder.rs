//! The kernel walks the ladder exactly as the host does: the same rungs,
//! the same bytes read to degrade, the same bytes held after, and the
//! same session terminated when no rung is left.
//!
//! The host side runs `mlos_ladder` over a manager built like the lab's
//! -- the synthetic capacity, the same arena size, the recompute tier --
//! and predicts every line the shell prints. Under TCG, as `replay.rs`
//! explains.

use std::process::Command;

use mlos_ladder::{arrive, squeeze};
use mlos_objman::{Arena, Contract, Manager};
use mlos_objtab::Precision;

/// The binary cargo built for this test.
const MLOS: &str = env!("CARGO_BIN_EXE_mlos");

/// Arena bytes, in kibibytes: 14 KiB unpromised once the working set is
/// reserved, which is seven tokens of context.
const BUDGET_KIB: u64 = 32;

/// How long to let the guest run.
const SECONDS: &str = "60";

/// Two sessions, `(FLOOR, CTX)`: one that will not go below Q8, and one
/// that accepts anything. Seven tokens between them, all the budget has.
const ASKS: [(&str, Precision, u32); 2] =
    [("q8", Precision::Q8, 3), ("ternary", Precision::Ternary, 4)];

/// Budgets to squeeze to, in KiB: L1 fits; then the Q8 session stops and
/// the other goes to L4; then nothing is left but terminating it.
const SQUEEZES: [u64; 3] = [30, 25, 22];

#[test]
#[ignore = "boots a VM; run with --ignored"]
fn the_kernel_walks_the_ladder_the_host_walks_on_aarch64() {
    agree("aarch64");
}

#[test]
#[ignore = "boots a VM; run with --ignored"]
fn the_kernel_walks_the_ladder_the_host_walks_on_x86_64() {
    agree("x86-64");
}

/// Boots `arch`, runs the script, and holds every line to the host's.
fn agree(arch: &str) {
    let console = guest(arch);
    let mut rest = console.as_str();
    for line in expected() {
        let found = rest.find(&line);
        assert!(
            found.is_some(),
            "{arch}: missing {line:?} in order, in:\n{console}"
        );
        rest = &rest[found.unwrap() + line.len()..];
    }
}

/// What the shell must print, in order, from the same code on the host.
fn expected() -> Vec<String> {
    let mut bytes = vec![0u8; (BUDGET_KIB << 10) as usize];
    let mut held: Manager<'_, 512> = Manager::new(Arena::new(&mut bytes));
    held.attach(&mlos_synth::tiers::RECOMPUTE).expect("attach");
    held.capacity = mlos_admit::Capacity {
        bytes: BUDGET_KIB << 10,
        ..mlos_lab::SYNTHETIC
    };
    let template = (mlos_lab::LAYERS, mlos_synth::kv::meta(0));
    let mut lines = Vec::new();
    for (_, floor, context) in ASKS {
        let contract = Contract {
            quality_floor: floor,
            context,
            ..Contract::NONE
        };
        let (id, _) = arrive(&mut held, &mlos_lab::LADDER, contract, template).expect("admitted");
        lines.push(format!("session {} created", id.0));
    }
    for kib in SQUEEZES {
        let done = squeeze(&mut held, &mlos_lab::LADDER, kib << 10);
        assert!(done.fits, "{kib} KiB should fit after the walk");
        lines.push(format!(
            "squeezed to {kib} KiB: reached {}, read {} B, {} terminated, fits",
            done.reached, done.recomputed, done.terminated
        ));
    }
    for s in held.sessions.each() {
        lines.push(format!("session {:<3} on {} ", s.id.0, s.rung));
        lines.push(format!(
            "read {} B, holds {} B",
            s.delivered.recomputed, s.resident
        ));
    }
    lines
}

/// Boots `arch` under TCG and runs the asks and the squeezes, then lists.
fn guest(arch: &str) -> String {
    let asks = ASKS.map(|(floor, _, context)| format!("session new 0 0 {floor} {context}"));
    let squeezes = SQUEEZES.map(|kib| format!("session squeeze {kib}"));
    let script = format!(
        "model {BUDGET_KIB};{};{};session",
        asks.join(";"),
        squeezes.join(";")
    );
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

#[test]
fn the_host_walk_reaches_every_kind_of_move() {
    let lines = expected();
    let squeezed: Vec<&String> = lines.iter().filter(|l| l.starts_with("squeezed")).collect();
    assert!(squeezed[0].contains("reached L1"), "{lines:?}");
    assert!(squeezed[1].contains("reached L4"), "{lines:?}");
    assert!(
        squeezed[2].contains("reached L7") && squeezed[2].contains("1 terminated"),
        "{lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.starts_with("session 1   on L1")),
        "{lines:?}"
    );
}
