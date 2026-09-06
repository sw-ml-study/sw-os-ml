//! `mlos` -- build, run and diagnose MLOS.
//!
//! Replaces `scripts/boot.sh` and `scripts/image.sh`, which were fine
//! until more than one thing needed them and a stale image started
//! producing confusing results.

mod doctor;
mod image;
mod run;

use std::{io, process::ExitCode};

/// Parses arguments, dispatches, and turns a failure into an exit code.
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match dispatch(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("mlos: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Runs one command.
fn dispatch(args: &[String]) -> io::Result<()> {
    let (host, seconds, debug) = options(args);
    match args.first().map(String::as_str) {
        Some("build") => println!("{}", image::build()?.display()),
        Some("run") => match seconds {
            Some(seconds) => run::capture(host, seconds)?,
            None => run::run(host, debug)?,
        },
        Some("doctor") => doctor::doctor(),
        Some("--version" | "-V") => println!("mlos {}", env!("CARGO_PKG_VERSION")),
        Some("--help" | "-h" | "help") | None => usage(),
        Some(other) => {
            usage();
            return Err(io::Error::other(format!("no such command: {other}")));
        }
    }
    Ok(())
}

/// Pulls the host, the capture duration and the debug flag out of the
/// arguments. Anything unrecognised is ignored rather than rejected --
/// this is a development tool, not a product.
fn options(args: &[String]) -> (&str, Option<u64>, bool) {
    let host = args.iter().skip(1).find(|arg| !arg.starts_with('-'));
    let seconds = args
        .iter()
        .position(|arg| arg == "--capture")
        .and_then(|at| args.get(at + 1))
        .and_then(|value| value.parse().ok());
    (
        host.map_or("hvf", String::as_str),
        seconds,
        args.iter().any(|a| a == "--debug"),
    )
}

/// Prints usage.
fn usage() {
    println!(
        "\
{} {} -- {}

Usage:
  mlos build              build the kernel and its bootable image
  mlos run [HOST]         boot it with the console on this terminal
  mlos doctor             report what is installed and what is missing

Arguments:
  HOST                    accelerator: hvf (default) or tcg

Options:
  --capture SECONDS       boot headless for SECONDS and print the console
  --debug                 halt at reset with a gdb stub on :1234
  -h, --help              print this help
  -V, --version           print version

The console is interactive: `mlsh` is on the other end. Quit with Ctrl-A x.",
        env!("CARGO_BIN_NAME"),
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_DESCRIPTION"),
    );
}
