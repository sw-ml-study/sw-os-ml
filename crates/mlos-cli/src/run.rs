//! Launching MLOS under a hypervisor.

use std::{fs, io, path::PathBuf, process::Command, thread, time::Duration};

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
pub fn run(host: &str, debug: bool, virtio: bool) -> io::Result<()> {
    let image = image::build()?;
    let (program, mut args) = command(host, &image.to_string_lossy(), None, virtio, "");
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

/// Boots headless for `seconds` and returns whatever the console said.
///
/// The non-interactive counterpart of [`run`], for tests and for checking
/// a change still boots. No keystroke reaches the guest -- a file is not a
/// terminal -- so a shell session is driven through `boot`, which becomes
/// `/chosen/bootargs` and can carry `mlsh.run=model;sweep;layout`. That is
/// what makes a runtime snapshot reproducible instead of something a
/// person has to sit and type.
pub fn capture(host: &str, seconds: u64, virtio: bool, boot: &str) -> io::Result<String> {
    let image = image::build()?;
    // Unique per invocation. A fixed name means two captures running at
    // once overwrite each other's console -- which is exactly what three
    // parallel boot tests do, and it looks like a kernel that sometimes
    // does not print.
    let log = std::env::temp_dir().join(format!("mlos-console-{}.txt", std::process::id()));
    let _ = fs::remove_file(&log);

    let (program, args) = command(
        host,
        &image.to_string_lossy(),
        Some(&log.to_string_lossy()),
        virtio,
        boot,
    );
    let mut child = Command::new(program).args(args).spawn()?;
    thread::sleep(Duration::from_secs(seconds));
    child.kill()?;
    child.wait()?;

    let console = fs::read_to_string(&log).unwrap_or_default();
    let _ = fs::remove_file(&log);
    Ok(console)
}

/// Boots, drives the shell, and writes the runtime layout it printed.
///
/// TCG rather than the default accelerator: this produces a file other
/// repositories render, and a snapshot that came out differently on
/// somebody else's machine would be worse than no snapshot. TCG is
/// deterministic, which is the property that matters here and the same
/// reason the boot tests use it.
/// Three artifacts from one boot: the snapshot, the events that led to it,
/// and the access trace those events record. Two boots would not be the
/// same run and nothing downstream could tell.
///
/// The trace is DERIVED from the events rather than recorded separately,
/// because every acquire is already in the stream and a second recorder
/// is a second thing to disagree with the first.
///
/// `mlsh.run=` goes last in the boot arguments: it takes the rest of the
/// string, because its commands take arguments and arguments have spaces.
///
/// The document goes through the same validator the static emitter uses.
/// Two emitters that share no code still have to produce one format, and
/// this is the only place that can tell -- the guest has no allocator to
/// check itself with, and the file is what other repositories read.
pub fn runtime(seconds: u64) -> io::Result<PathBuf> {
    use mlos_image_map::runtime::{EVENTS, OUT, SCRIPT, TRACE, events, save, trace};
    let script = format!("mlos.rev={} mlsh.run={SCRIPT}", mlos_image_map::revision());
    let console = capture("tcg", seconds, false, &script)?;
    let document = mlos_image_map::runtime::extract(&console)?;
    mlos_layout::validate(&document).map_err(io::Error::other)?;

    let out = save(OUT, &document)?;
    let stream = events(&console);
    save(EVENTS, &stream)?;
    save(TRACE, &trace(&stream)?)?;
    Ok(out)
}

/// Boots, headless or interactive, according to the arguments.
pub fn boot(args: &[String]) -> io::Result<()> {
    let (host, seconds, debug) = crate::options(args, true)?;
    // `--console virtio`; the parser in main.rs accepts the pair.
    let virtio = args.iter().any(|arg| arg == "virtio");
    match seconds {
        Some(seconds) => {
            print!("{}", capture(host, seconds, virtio, "")?);
            Ok(())
        }
        None => run(host, debug, virtio),
    }
}
