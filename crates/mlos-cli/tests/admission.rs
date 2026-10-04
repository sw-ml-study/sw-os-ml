//! The kernel admits and refuses exactly what the host-side arithmetic
//! says it should, reason by reason, and reports the same `Ss`.
//!
//! Both sides link `mlos-admit` and build the synthetic model's capacity
//! from the same constants, so every line the shell prints for a
//! `session new` is predicted here first. Under TCG, as `replay.rs`
//! explains.

use std::process::Command;

use mlos_admit::Capacity;
use mlos_session::{Contract, MAX_SESSIONS, Sessions};

/// The binary cargo built for this test.
const MLOS: &str = env!("CARGO_BIN_EXE_mlos");

/// Arena bytes, in kibibytes: 14 KiB unpromised once the working set is
/// reserved, so the byte rule bites on the second session of four tokens.
const BUDGET_KIB: u64 = 32;

/// How long to let the guest run: a handful of syscalls, with room.
const SECONDS: &str = "60";

/// What is asked for, as `(KIB, WAIT, CTX)`, chosen so every rule refuses
/// once: a ceiling below its own context, the bytes running out, a
/// ceiling under a quarter token, and a ceiling the others could not keep.
const ASKS: [(u64, u32, u32); 7] = [
    (0, 0, 4),
    (4, 0, 4),
    (0, 0, 4),
    (0, 16, 0),
    (0, 200, 1),
    (0, 100, 0),
    (0, 300, 0),
];

#[test]
#[ignore = "boots a VM; run with --ignored"]
fn the_kernel_admits_what_the_host_admits_on_aarch64() {
    agree("aarch64");
}

#[test]
#[ignore = "boots a VM; run with --ignored"]
fn the_kernel_admits_what_the_host_admits_on_x86_64() {
    agree("x86-64");
}

/// Boots `arch`, runs the asks, and holds every answer to the host's.
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

/// What the shell must print, in order, by the same arithmetic.
fn expected() -> Vec<String> {
    let capacity = Capacity {
        bytes: BUDGET_KIB << 10,
        ..mlos_lab::SYNTHETIC
    };
    let mut live = Sessions::<MAX_SESSIONS>::EMPTY;
    let mut lines: Vec<String> = ASKS
        .iter()
        .map(|&(kib, wait, context)| {
            let contract = Contract {
                resident_ceiling: kib << 10,
                latency_ceiling: wait,
                context,
                ..Contract::NONE
            };
            match capacity.admit(&live, &contract) {
                Ok(()) => format!("session {} created", live.create(contract).expect("room").0),
                Err(why) => format!("refused: {why}"),
            }
        })
        .collect();
    let admits = capacity.admits(&live);
    lines.push(format!("budget admits {admits} session(s) like these"));
    lines
}

/// Boots `arch` under TCG and runs the asks, then lists.
fn guest(arch: &str) -> String {
    let asks = ASKS
        .iter()
        .map(|(kib, wait, context)| format!("session new {kib} {wait} ternary {context}"))
        .collect::<Vec<_>>()
        .join(";");
    let script = format!("model {BUDGET_KIB};{asks};session");
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
