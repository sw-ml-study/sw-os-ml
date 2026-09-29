//! `mlos` -- build, run and diagnose MLOS.
//!
//! Invariant: an argument the parser does not recognise is rejected here,
//! before it can reach a VMM. Design and history: docs/notes/mlos-cli.md.

mod doctor;
mod image;
mod run;
mod vmm;
mod x86;

use std::{io, process::ExitCode};

/// Parses arguments, dispatches, and turns a failure into an exit code.
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match x86::dispatch(&args).unwrap_or_else(|| dispatch(&args)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("mlos: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Runs one command.
fn dispatch(args: &[String]) -> io::Result<()> {
    let verb = args.first().map(String::as_str);
    // Every verb but `run` takes no positional argument, so they are all
    // validated in one place rather than each remembering to.
    if matches!(verb, Some("build" | "layout" | "runtime" | "doctor")) {
        options(args, false)?;
    }
    match verb {
        Some("build") => println!("{}", image::build()?.display()),
        Some("run") => run::boot(args)?,
        Some("runtime") => println!("{}", run::runtime(RUNTIME_SECONDS)?.display()),
        Some("layout") => {
            let written = mlos_image_map::emit(&image::build()?, &image::disk()?)?;
            println!("{}", written.display());
        }
        Some("doctor") => doctor::doctor(),
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
/// arguments, rejecting anything it does not recognise. `takes_host` is
/// false for verbs with no positional argument. The values of `--console`
/// and `--run` are skipped, not returned: `run::boot` reads them back
/// positionally, and neither may reach the accelerator check.
fn options(args: &[String], takes_host: bool) -> io::Result<(&str, Option<u64>, bool)> {
    let mut rest = args.iter().skip(1);
    let (mut host, mut seconds, mut debug) = (None, None, false);

    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--debug" => debug = true,
            "--console" | "--run" => drop(rest.next()),
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

/// Checks an accelerator name against `run::HOSTS` before it can reach
/// QEMU.
fn accelerator(name: &str) -> io::Result<&str> {
    if run::HOSTS.contains(&name) {
        return Ok(name);
    }
    Err(io::Error::other(format!(
        "unknown accelerator {name:?} (expected one of: {})",
        run::HOSTS.join(", ")
    )))
}

/// How long to let the guest run before reading its console. Must cover
/// a whole sweep under TCG: a snapshot cut off half way is a valid
/// document of a system that never existed.
const RUNTIME_SECONDS: u64 = 12;

/// What `mlos --help` prints.
const USAGE: &str = "\
mlos -- build, run and diagnose MLOS

Usage:
  mlos build              build the kernel and its bootable image
  mlos run [HOST]         boot it with the console on this terminal
  mlos layout             write build/storage-layout.json from the build
  mlos runtime            boot, sweep, and write build/runtime-layout.json
  mlos doctor             report what is installed and what is missing

Arguments:
  HOST                    accelerator: hvf (default) or tcg

Options:
  --capture SECONDS       boot headless for SECONDS and print the console
  --run SCRIPT            drive mlsh with SCRIPT, as `model 32;replay lru`
  --debug                 halt at reset with a gdb stub on :1234
  -h, --help              print this help
  -V, --version           print version

The console is interactive: `mlsh` is on the other end. Quit with Ctrl-A x.";
