//! What is installed, what is missing, and what that stops.
//!
//! Invariant: a missing tool is the answer, never an error. Design:
//! docs/notes/mlos-cli.md.

use std::process::Command;

use crate::{image, run};

/// Tools to look for, grouped by section, each with the flag that makes
/// it report a version (`ffmpeg` wants a single dash).
const TOOLS: [(&str, &[(&str, &str)]); 3] = [
    (
        "toolchain",
        &[("rustc", "--version"), ("cargo", "--version")],
    ),
    (
        "emulator",
        &[
            ("qemu-system-aarch64", "--version"),
            ("qemu-system-x86_64", "--version"),
            ("vfkit", "--version"),
        ],
    ),
    (
        "demo recording (optional)",
        &[
            ("vhs", "--version"),
            ("ttyd", "--version"),
            ("ffmpeg", "-version"),
            ("gifsicle", "--version"),
            ("gif2webp", "-version"),
        ],
    ),
];

/// Bare-metal targets the kernel needs.
const TARGETS: [&str; 2] = [image::TARGET, "x86_64-unknown-none"];

/// Prints a report of the host's tooling. Never fails.
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
/// Accelerators come from `run::HOSTS`, so the two cannot drift apart.
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
/// there. All of it: some answers are lists.
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
