//! Keeps the README's commands working.
//!
//! Written because they stopped: a line in the README was copied into an
//! interactive zsh, where `#` is not a comment, and the trailing note
//! became arguments. Documentation that does not run is a bug report
//! waiting to be filed by whoever tries it.
//!
//! Fast on purpose -- no VM, no QEMU -- so it runs in the ordinary test
//! gate rather than being something to remember.

use std::process::Command;

/// The binary cargo built for this test.
const MLOS: &str = env!("CARGO_BIN_EXE_mlos");

/// The README, read at compile time so a missing file fails the build.
const README: &str = include_str!("../../../README.md");

/// Every `cargo run -p mlos-cli -- X` in the README, reduced to its
/// subcommand.
fn documented_commands() -> Vec<&'static str> {
    README
        .lines()
        .filter_map(|line| line.trim().strip_prefix("cargo run -p mlos-cli -- "))
        .filter_map(|rest| rest.split_whitespace().next())
        .collect()
}

/// Runs `mlos` and returns its exit status and combined output.
fn mlos(args: &[&str]) -> (bool, String) {
    let out = Command::new(MLOS).args(args).output().expect("mlos runs");
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), text)
}

/// The README must actually offer commands, or this file passes by
/// checking nothing -- which is how a documentation test rots.
#[test]
fn the_readme_shows_some_commands() {
    let commands = documented_commands();
    assert!(
        !commands.is_empty(),
        "no `cargo run -p mlos-cli --` lines found"
    );
    assert!(
        commands.contains(&"run"),
        "the README should show how to boot it"
    );
}

/// Every command the README shows is one `mlos` actually has.
#[test]
fn every_documented_command_exists() {
    let (ok, help) = mlos(&["--help"]);
    assert!(ok, "--help should succeed");
    for command in documented_commands() {
        assert!(
            help.contains(&format!("mlos {command}")),
            "{command} is not in --help"
        );
    }
}

/// `doctor` is the one command safe to run here: it starts no VM and
/// reports rather than fails.
#[test]
fn doctor_runs_and_reports() {
    let (ok, report) = mlos(&["doctor"]);
    assert!(ok, "doctor should succeed: {report}");
    assert!(
        report.contains("qemu-system-aarch64"),
        "doctor should look for qemu"
    );
}

/// A pasted `# comment` must be refused, not forwarded. This is the exact
/// shape that reached QEMU as an accelerator name.
#[test]
fn a_pasted_comment_is_refused() {
    let (ok, message) = mlos(&["run", "#", "boot", "it"]);
    assert!(!ok, "a stray argument must fail");
    assert!(
        message.contains("unknown accelerator"),
        "should name the problem: {message}"
    );
}

/// The README must not itself carry a trailing `#` comment on a command
/// line, because interactive zsh does not strip it.
#[test]
fn no_command_in_the_readme_carries_a_trailing_comment() {
    for line in README.lines().map(str::trim) {
        if line.starts_with("cargo run -p mlos-cli") || line.starts_with("mlos ") {
            assert!(
                !line.contains(" #"),
                "trailing comment will become arguments: {line}"
            );
        }
    }
}

/// The tracked demo must stay small.
///
/// `vhs` writes a 25 fps GIF of a mostly static terminal, which is three
/// times larger than it needs to be. Optimising is one command, and a
/// forgotten one grows the repository quietly and permanently -- git keeps
/// every version of a binary forever. So the budget is enforced here
/// rather than remembered.
///
/// WebP was measured and is worse for this content: lossy WebP spends
/// bits on gradients a terminal recording does not have.
#[test]
fn the_readme_demo_stays_within_budget() {
    const BUDGET: u64 = 40 * 1024;
    let gif = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/tour.gif");
    let size = std::fs::metadata(gif).expect("docs/tour.gif exists").len();

    assert!(
        size <= BUDGET,
        "docs/tour.gif is {size} bytes, over the {BUDGET} budget. Run:\n  \
         gifsicle -O3 --colors 16 --lossy=60 docs/tour.gif -o docs/tour.gif"
    );
}
