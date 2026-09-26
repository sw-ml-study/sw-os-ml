//! `--arch x86-64`: building and booting the x86-64 guest.
//!
//! Its own module, reached through one hook in `main`, so the x86-64 lane
//! (saga `mlos-x86-64`, `lanes/x86/`) and the aarch64 lane never edit the
//! same lines. Folding the two into one `--arch`-aware path is a later
//! refactor, once both have merged.
//!
//! Without `--arch`, nothing changes: `mlos` means aarch64, as before.
//! `--arch aarch64` says so explicitly and takes the unchanged path.

use std::{
    io,
    path::PathBuf,
    process::Command,
    thread,
    time::{Duration, Instant},
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
  --capture SECONDS               boot headless, give up after SECONDS

TCG only: KVM is saga mlos-two-hosts. There is no console yet (step
x86-console); the guest reports what it checked through its exit status.";

/// Handles `--arch`, or returns `None` so `main` takes the usual path.
///
/// `--arch aarch64` is stripped and handed back to [`crate::dispatch`], so
/// both architectures are selectable and neither is reached by accident.
pub fn dispatch(args: &[String]) -> Option<io::Result<()>> {
    let at = args.iter().position(|arg| arg == "--arch")?;
    let mut rest = args.to_vec();
    let arch = rest
        .drain(at..args.len().min(at + 2))
        .nth(1)
        .unwrap_or_default();
    Some(match (arch.as_str(), rest.first().map(String::as_str)) {
        ("aarch64", _) => crate::dispatch(&rest),
        ("x86-64", Some("build")) => build().map(|elf| println!("{}", elf.display())),
        ("x86-64", Some("run")) => boot(&rest),
        ("x86-64", None | Some("help" | "--help" | "-h")) => {
            println!("{USAGE}");
            Ok(())
        }
        ("x86-64", Some(other)) => Err(io::Error::other(format!(
            "{other:?} is not available for x86-64 yet (see `mlos --arch x86-64 help`)"
        ))),
        (other, _) => Err(io::Error::other(format!(
            "unknown architecture {other:?} (expected aarch64 or x86-64)"
        ))),
    })
}

/// Builds the kernel for x86-64 and returns the ELF.
///
/// The ELF itself, not a flat image: PVH is found through an ELF note, so
/// QEMU needs the program headers an objcopy would strip.
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

/// QEMU for the x86-64 guest, minus RAM size and kernel.
///
/// `microvm`: virtio-mmio, which MLOS speaks, rather than `q35`'s PCI.
/// `-cpu max` so the identity map can use 1 GiB pages; the 2 MiB fallback
/// is exercised by the boot test with QEMU's default CPU. The debug-exit
/// device is how the guest reports before it has a console.
#[rustfmt::skip] // flag/value pairs, one pair per line reads as a command line
const MICROVM: [&str; 15] = [
    "-M", "microvm", "-accel", "tcg", "-cpu", "max",
    "-display", "none", "-nodefaults", "-no-reboot", "-serial", "mon:stdio",
    "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04", "-kernel",
];

/// Boots under QEMU `microvm` and reports what the guest said it checked.
fn boot(args: &[String]) -> io::Result<()> {
    let (host, seconds, _) = crate::options(args, true)?;
    if args.iter().any(|arg| arg == host) && host != "tcg" {
        return Err(io::Error::other(format!(
            "{host} cannot run the x86-64 guest here; saga mlos-x86-64 is TCG-only"
        )));
    }
    let ram = (mlos_image_map::RAM_BYTES >> 20).to_string();
    let mut child = Command::new("qemu-system-x86_64")
        .args(["-m", &ram])
        .args(MICROVM)
        .arg(build()?)
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(seconds.unwrap_or(u64::MAX >> 1));
    while child.try_wait()?.is_none() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    println!("{}", describe(child.wait()?.code())?);
    Ok(())
}

/// Turns the guest's exit status into what it means, or into an error.
///
/// QEMU's `isa-debug-exit` exits with `(code << 1) | 1`. The bits are
/// `mlos-kernel/src/x86.rs`'s: `0x40` reached `mlos_main`, `0x01` long
/// mode, `0x02` a valid `hvm_start_info`, `0x04` 1 GiB pages.
fn describe(status: Option<i32>) -> io::Result<String> {
    let code = status.ok_or_else(|| io::Error::other("the guest did not exit in time"))?;
    let bits = (code - 1) / 2;
    if code % 2 == 0 || bits & 0x40 == 0 {
        return Err(io::Error::other(format!(
            "QEMU exited {code} without a guest report"
        )));
    }
    let yes =
        |bit: i32, on: &'static str, off: &'static str| if bits & bit != 0 { on } else { off };
    let report = format!(
        "x86-64 guest reached mlos_main: long mode {}, start_info {}, identity map {} pages",
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
