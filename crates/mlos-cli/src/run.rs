//! Launching MLOS under a hypervisor.

use std::{fs, io, process::Command, thread, time::Duration};

use crate::{image, vmm::command};

/// The hypervisors `mlos run` knows about.
///
/// `hvf` and `tcg` are QEMU accelerators; `vz` is a different program
/// entirely -- Apple's Virtualization.framework, driven through vfkit.
/// They share a list because from the outside they are the same question:
/// which thing runs MLOS.
pub const HOSTS: [&str; 3] = ["hvf", "tcg", "vz"];

/// Boots the kernel with the console attached to this terminal.
///
/// Interactive: `mlsh` is on the other end, and a piped stdin would not
/// reach it -- a receive FIFO only sees keystrokes from a real terminal.
/// Quit with `Ctrl-A x` under QEMU, `Ctrl-C` under vfkit.
///
/// **`vz` produces no output yet.** Virtualization.framework offers a
/// virtio console and no PL011, and MLOS drives only the latter. The VM
/// runs; nothing says so. Requirement N2 closes when the virtio-console
/// driver lands.
pub fn run(host: &str, debug: bool) -> io::Result<()> {
    let image = image::build()?;
    let (program, mut args) = command(host, &image.to_string_lossy(), None);
    if debug {
        // Halted, with the stub open. `target remote :1234` in gdb, then
        // load the ELF for symbols -- the image has none.
        args.extend(["-S".to_owned(), "-gdb".to_owned(), "tcp::1234".to_owned()]);
        eprintln!("gdb stub on :1234, cpu halted; `target remote :1234` to attach");
    }
    eprintln!("quit with Ctrl-A x");
    let status = Command::new(program).args(args).status()?;
    if status.success() {
        return Ok(());
    }
    Err(io::Error::other(format!("{program} exited: {status}")))
}

/// Boots headless for `seconds`, then prints whatever the console said.
///
/// The non-interactive counterpart of [`run`], for tests and for checking
/// a change still boots. It cannot exercise the shell -- a file is not a
/// terminal, so no keystroke ever reaches the guest -- which is why
/// `demos/*.tape` exists and drives a real pty instead.
pub fn capture(host: &str, seconds: u64) -> io::Result<()> {
    let image = image::build()?;
    let log = std::env::temp_dir().join("mlos-console.txt");
    let _ = fs::remove_file(&log);

    let (program, args) = command(host, &image.to_string_lossy(), Some(&log.to_string_lossy()));
    let mut child = Command::new(program).args(args).spawn()?;
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
