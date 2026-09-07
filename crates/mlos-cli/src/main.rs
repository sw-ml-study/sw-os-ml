//! `mlos` -- build, run and diagnose MLOS.
//!
//! Replaces `scripts/boot.sh` and `scripts/image.sh`, which were fine
//! until more than one thing needed them and a stale image started
//! producing confusing results.

mod doctor;
mod image;
mod run;
mod vmm;

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
    match args.first().map(String::as_str) {
        Some("build") => {
            options(args, false)?;
            println!("{}", image::build()?.display());
        }
        Some("run") => run::boot(args)?,
        Some("doctor") => {
            options(args, false)?;
            doctor::doctor();
        }
        Some("--version" | "-V") => println!("mlos {}", env!("CARGO_PKG_VERSION")),
        Some("--help" | "-h" | "help") | None => println!("{USAGE}"),
        Some(other) => {
            println!("{USAGE}");
            return Err(io::Error::other(format!("no such command: {other}")));
        }
    }
    Ok(())
}

/// Pulls the host, the capture duration and the debug flag out of the
/// arguments, rejecting anything it does not recognise.
///
/// `takes_host` is false for commands that accept no positional argument,
/// so `mlos doctor oops` is told it passed an unexpected argument rather
/// than that it chose a bad accelerator.
///
/// Rejecting, not ignoring. A tolerant parser turns a typo into a
/// confusing failure somewhere further down: a stray argument once reached
/// QEMU as an accelerator name, and the error the user saw came from a
/// program they had not typed.
fn options(args: &[String], takes_host: bool) -> io::Result<(&str, Option<u64>, bool)> {
    let mut rest = args.iter().skip(1);
    let (mut host, mut seconds, mut debug) = (None, None, false);

    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--debug" => debug = true,
            // `--console virtio` selects the virtio console. Its value is
            // consumed here so it is not mistaken for an accelerator.
            "--console" => drop(rest.next()),
            "--capture" => {
                let value = rest.next().and_then(|seconds| seconds.parse().ok());
                seconds = Some(
                    value.ok_or_else(|| io::Error::other("--capture needs a number of seconds"))?,
                );
            }
            other if other.starts_with('-') => {
                return Err(io::Error::other(format!("unknown option {other:?}")));
            }
            other if takes_host && host.is_none() => host = Some(accelerator(other)?),
            other => return Err(io::Error::other(format!("unexpected argument {other:?}"))),
        }
    }
    Ok((host.unwrap_or("hvf"), seconds, debug))
}

/// Checks an accelerator name before it can reach QEMU.
///
/// Worth doing rather than letting QEMU object, because QEMU objects to
/// the wrong thing: a stray `#` arrived here as an accelerator, and what
/// the user saw was `invalid accelerator #` from a program they had never
/// typed.
fn accelerator(name: &str) -> io::Result<&str> {
    if run::HOSTS.contains(&name) {
        return Ok(name);
    }
    Err(io::Error::other(format!(
        "unknown accelerator {name:?} (expected one of: {})",
        run::HOSTS.join(", ")
    )))
}

/// What `mlos --help` prints.
const USAGE: &str = "\
mlos -- build, run and diagnose MLOS

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

The console is interactive: `mlsh` is on the other end. Quit with Ctrl-A x.";
