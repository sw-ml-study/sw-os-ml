//! What is installed, what is missing, and what that stops.
//!
//! Earns its place because the host requirements differ sharply between a
//! Mac and the Linux/NVIDIA box, and "why will it not boot" should be
//! answerable by a command rather than by rereading
//! `docs/architecture.md` s.8.

use std::process::Command;

use crate::{image, run};

/// Tools to look for, grouped by section.
///
/// A table so that adding a check is a row rather than a code change.
/// `ffmpeg` wants a single dash, and reporting "missing" because the flag
/// was wrong is exactly what `doctor` must not do.
const TOOLS: [(&str, &[(&str, &str)]); 3] = [
    (
        "toolchain",
        &[("rustc", "--version"), ("cargo", "--version")],
    ),
    ("emulator", &[("qemu-system-aarch64", "--version")]),
    (
        "demo recording (optional)",
        &[
            ("vhs", "--version"),
            ("ttyd", "--version"),
            ("ffmpeg", "-version"),
        ],
    ),
];

/// Bare-metal targets the kernel needs.
const TARGETS: [&str; 2] = [image::TARGET, "x86_64-unknown-none"];

/// Prints a report of the host's tooling.
///
/// Never fails: a missing tool is the answer, not an error.
pub fn doctor() {
    for (section, entries) in TOOLS {
        println!("{section}");
        for (tool, flag) in entries {
            report(tool, &probe(tool, &[flag]));
        }
        println!();
    }
    inventory();

    println!("\nimage tools");
    let objcopy = image::objcopy().ok().filter(|path| path.exists());
    report(
        "llvm-objcopy",
        &objcopy.map(|path| path.display().to_string()),
    );
}

/// The checks whose answer is "does this name appear in that list".
///
/// Accelerators come from `run`'s own list rather than a copy, so the two
/// cannot drift apart.
fn inventory() {
    let listed = |haystack: &Option<String>, wanted: &[&str], note: &str| {
        for name in wanted {
            let found = haystack.as_deref().is_some_and(|list| list.contains(*name));
            report(name, &found.then(|| note.to_owned()));
        }
    };

    println!("kernel targets");
    listed(
        &probe("rustup", &["target", "list", "--installed"]),
        &TARGETS,
        "installed",
    );

    println!("\naccelerators");
    listed(
        &probe("qemu-system-aarch64", &["-accel", "help"]),
        &run::HOSTS,
        "available",
    );
}

/// Runs a tool and returns all of its output, or `None` if it is not
/// there.
///
/// All of it, not the first line: `rustup target list --installed` and
/// `qemu -accel help` both answer with a list, and checking only the first
/// line of a list reports everything after it as missing.
fn probe(tool: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(tool).args(args).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    (!text.is_empty()).then_some(text)
}

/// One line of the report, showing only the first line of what was found.
fn report(name: &str, found: &Option<String>) {
    match found {
        Some(detail) => {
            let summary = detail.lines().next().unwrap_or(detail);
            println!("  ok      {name:<28} {summary}");
        }
        None => println!("  MISSING {name}"),
    }
}
