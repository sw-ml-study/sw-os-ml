//! Launching MLOS under a hypervisor.

use std::{fs, io, process::Command, thread, time::Duration};

use crate::image;

/// The accelerators `mlos run` knows about.
pub const HOSTS: [&str; 2] = ["hvf", "tcg"];

/// Boots the kernel with the console attached to this terminal.
///
/// Interactive: `mlsh` is on the other end, and a piped stdin would not
/// reach it -- the PL011's receive FIFO only sees keystrokes from a real
/// terminal. Quit with `Ctrl-A x`.
pub fn run(host: &str, debug: bool) -> io::Result<()> {
    let image = image::build()?;
    let mut qemu = Command::new("qemu-system-aarch64");
    qemu.args(base_args(host));
    qemu.args(["-kernel", &image.to_string_lossy()]);
    qemu.args(["-serial", "mon:stdio"]);
    if debug {
        // Halted, with the stub open. `target remote :1234` in gdb, then
        // load the ELF for symbols -- the image has none.
        qemu.args(["-S", "-gdb", "tcp::1234"]);
        eprintln!("gdb stub on :1234, cpu halted; `target remote :1234` to attach");
    }
    eprintln!("quit with Ctrl-A x");

    let status = qemu.status()?;
    if status.success() {
        return Ok(());
    }
    Err(io::Error::other(format!("qemu exited: {status}")))
}

/// The machine MLOS expects.
///
/// `gic-version=3` is pinned rather than left to QEMU: its default differs
/// by accelerator -- TCG gives a GICv2, HVF a v3 -- so without this the two
/// hosts hand the guest different interrupt controllers, and only one of
/// them is the one MLOS drives.
///
/// The serial device is left to the caller: [`run`] multiplexes it with
/// the monitor onto this terminal, which is what makes `Ctrl-A x` work,
/// while [`capture`] sends it to a file.
fn base_args(host: &str) -> Vec<String> {
    let cpu = if host == "hvf" { "host" } else { "cortex-a72" };
    [
        "-M",
        "virt,gic-version=3",
        "-cpu",
        cpu,
        "-accel",
        host,
        "-m",
        "512",
        "-display",
        "none",
    ]
    .iter()
    .map(|arg| (*arg).to_owned())
    .collect()
}

/// Boots headless for `seconds`, then prints whatever the console said.
///
/// The non-interactive counterpart of [`run`], for tests and for checking
/// a change still boots. It cannot exercise the shell -- a file is not a
/// terminal, so the receive FIFO never sees a keystroke -- which is why
/// `demos/*.tape` exists and drives a real pty instead.
pub fn capture(host: &str, seconds: u64) -> io::Result<()> {
    let image = image::build()?;
    let log = std::env::temp_dir().join("mlos-console.txt");
    let _ = fs::remove_file(&log);

    let mut child = Command::new("qemu-system-aarch64")
        .args(base_args(host))
        .args(["-kernel", &image.to_string_lossy()])
        .args([
            "-serial",
            &format!("file:{}", log.display()),
            "-monitor",
            "none",
        ])
        .spawn()?;

    thread::sleep(Duration::from_secs(seconds));
    child.kill()?;
    child.wait()?;

    print!("{}", fs::read_to_string(&log).unwrap_or_default());
    Ok(())
}

/// Boots, headless or interactive, according to the arguments.
pub fn boot(args: &[String]) -> io::Result<()> {
    let (host, seconds, debug) = crate::options(args, true)?;
    match seconds {
        Some(seconds) => capture(host, seconds),
        None => run(host, debug),
    }
}
