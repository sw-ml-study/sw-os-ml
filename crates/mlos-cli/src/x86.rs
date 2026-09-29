//! `--arch x86-64`: building and booting the x86-64 guest.
//!
//! Invariant: reached through one hook in `main`, so the x86-64 and
//! aarch64 lanes never edit the same lines. Without `--arch`, `build` and
//! `run` target the host's own architecture. Design and history:
//! docs/notes/mlos-cli.md.

use std::{
    io::{self, Write},
    path::PathBuf,
    process::Command,
    thread,
    time::{Duration, Instant},
};

/// The architecture `build` and `run` mean when `--arch` is not given:
/// this machine's, as `mlos` itself was compiled for it.
const HOST: &str = if cfg!(target_arch = "x86_64") {
    "x86-64"
} else {
    "aarch64"
};

/// The bare target the x86-64 kernel is built for.
const TARGET: &str = "x86_64-unknown-none";

/// What `mlos --arch x86-64 help` prints.
const USAGE: &str = "\
mlos --arch x86-64 -- the x86-64 guest (saga mlos-x86-64, in progress)

Usage:
  mlos --arch x86-64 build        build the x86-64 kernel ELF
  mlos --arch x86-64 run [tcg]    boot it under QEMU microvm via PVH

Options:
  --capture SECONDS               boot headless for SECONDS, print the console
  --run SCRIPT                    drive mlsh with SCRIPT, as `model 32;get 0 0`

On an x86-64 host `--arch x86-64` is the default for build and run.
The console is COM1 with mlsh on it (try `mem`, `dev`, `model`); the
model disk is attached over virtio-mmio. Ctrl-D ends the guest, which then reports what it checked through its exit status. TCG
only: KVM is saga mlos-two-hosts.";

/// Handles `--arch`, or an x86-64 host's `build`/`run`; `None` means
/// `main` takes the usual path.
///
/// `--arch aarch64` is stripped and handed back to [`crate::dispatch`], so
/// both architectures are selectable wherever `mlos` runs.
pub fn dispatch(args: &[String]) -> Option<io::Result<()>> {
    let mut rest = args.to_vec();
    let arch = match args.iter().position(|arg| arg == "--arch") {
        Some(at) => rest
            .drain(at..args.len().min(at + 2))
            .nth(1)
            .unwrap_or_default(),
        None if matches!(args.first().map(String::as_str), Some("build" | "run")) => HOST.into(),
        None => return None,
    };
    Some(match (arch.as_str(), rest.first().map(String::as_str)) {
        ("aarch64", _) => crate::dispatch(&rest),
        ("x86-64", Some("build")) => build().map(|elf| println!("{}", elf.display())),
        ("x86-64", Some("run")) => boot(&rest),
        ("x86-64", None | Some("help" | "--help" | "-h")) => writeln!(io::stdout(), "{USAGE}"),
        ("x86-64", Some(other)) => Err(io::Error::other(format!(
            "{other:?} is not available for x86-64 yet (see `mlos --arch x86-64 help`)"
        ))),
        (other, _) => Err(io::Error::other(format!(
            "unknown architecture {other:?} (expected aarch64 or x86-64)"
        ))),
    })
}

/// Builds the kernel for x86-64 and returns the ELF itself, not a flat
/// image: PVH is found through an ELF note, so QEMU needs the program
/// headers.
fn build() -> io::Result<PathBuf> {
    let args = ["build", "-q", "-p", "mlos-kernel", "--target", TARGET];
    let status = Command::new("cargo").args(args).status()?;
    if !status.success() {
        return Err(io::Error::other(format!("cargo failed: {status}")));
    }
    Ok(PathBuf::from("target")
        .join(TARGET)
        .join("debug/mlos-kernel"))
}

/// QEMU for the x86-64 guest, minus RAM size and kernel. `acpi=off` keeps
/// the virtio-mmio slots on the command line, where `mlos-pvh` reads
/// them; `-cpu max` allows 1 GiB pages; `isa-debug-exit` is how the
/// guest reports before it has a console.
#[rustfmt::skip] // flag/value pairs, one pair per line reads as a command line
const MICROVM: [&str; 15] = [
    "-M", "microvm,acpi=off", "-accel", "tcg", "-cpu", "max",
    "-display", "none", "-nodefaults", "-no-reboot", "-serial", "mon:stdio",
    "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04", "-kernel",
];

/// Boots under QEMU `microvm`, console on this terminal. With
/// `--capture`, kills the guest at the deadline (still running is the
/// normal end); without it, waits for the guest. `--run SCRIPT` becomes
/// `mlsh.run=`.
fn boot(args: &[String]) -> io::Result<()> {
    let (host, seconds, _) = crate::options(args, true)?;
    if args.iter().any(|arg| arg == host) && host != "tcg" {
        return Err(io::Error::other(format!(
            "{host} cannot run the x86-64 guest here; saga mlos-x86-64 is TCG-only"
        )));
    }
    let script = args.iter().skip_while(|arg| *arg != "--run").nth(1);
    let append = script.map_or_else(String::new, |script| format!("mlsh.run={script}"));
    let ram = (mlos_image_map::RAM_BYTES >> 20).to_string();
    let mut child = Command::new("qemu-system-x86_64")
        .args(["-m", &ram, "-append", &append])
        .args(crate::vmm::disk())
        .args(MICROVM) // last: it ends in `-kernel`, which takes the ELF
        .arg(build()?)
        .spawn()?;
    let deadline = seconds.map(|s| Instant::now() + Duration::from_secs(s));
    while child.try_wait()?.is_none() && deadline.is_none_or(|end| Instant::now() < end) {
        thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    println!("\n{}", describe(child.wait()?.code())?);
    Ok(())
}

/// Turns the guest's exit status into what it means, or into an error.
/// `None` is the `--capture` deadline, not a failure. Otherwise the code
/// is `(bits << 1) | 1` from `isa-debug-exit`, bits as `mlos-kernel-x86-64`
/// sets them: `0x40` reached `mlos_main`, `0x01` long mode, `0x02` valid
/// `hvm_start_info`, `0x04` 1 GiB pages, `0x08` stopped by a trap.
fn describe(status: Option<i32>) -> io::Result<String> {
    let Some(code) = status else {
        return Ok("(stopped at the --capture deadline)".to_owned());
    };
    let bits = (code - 1) / 2;
    if code % 2 == 0 || bits & 0x40 == 0 {
        return Err(io::Error::other(format!(
            "QEMU exited {code} without a guest report"
        )));
    }
    let yes =
        |bit: i32, on: &'static str, off: &'static str| if bits & bit != 0 { on } else { off };
    let report = format!(
        "x86-64 guest {}: long mode {}, start_info {}, identity map {} pages",
        yes(0x08, "stopped by a trap", "exited"),
        yes(0x01, "yes", "NO"),
        yes(0x02, "valid", "INVALID"),
        yes(0x04, "1 GiB", "2 MiB")
    );
    if bits & 0x03 == 0x03 {
        Ok(report)
    } else {
        Err(io::Error::other(report))
    }
}
