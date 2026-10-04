//! Launching MLOS under a hypervisor.
//!
//! Invariant: `mlsh.run=` goes last in the boot arguments, because it
//! takes the rest of the string. Design and history:
//! docs/notes/mlos-cli.md.

use std::{fs, io, path::PathBuf, process::Command, thread, time::Duration};

use crate::{image, vmm::command};

/// The hypervisors `mlos run` knows about: `hvf` and `tcg` are QEMU
/// accelerators, `vz` is vfkit.
pub const HOSTS: [&str; 3] = ["hvf", "tcg", "vz"];

/// Boots the kernel with the console attached to this terminal. `mlsh`
/// is on the other end, so stdin must be a real terminal, not a pipe.
/// `vz` produces no output yet: MLOS has no virtio-console driver.
pub fn run(host: &str, debug: bool, virtio: bool, boot: &str) -> io::Result<()> {
    let image = image::build()?;
    let (program, mut args) = command(host, &image.to_string_lossy(), None, virtio, boot);
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
/// `boot` becomes `/chosen/bootargs`. The log is named per process so
/// parallel captures do not collide, and the emulator's stderr is kept:
/// exiting with an empty console is the failure, an empty console alone
/// is not.
pub fn capture(host: &str, seconds: u64, virtio: bool, boot: &str) -> io::Result<String> {
    let image = image::build()?;
    let log = std::env::temp_dir().join(format!("mlos-console-{}.txt", std::process::id()));
    let errors = log.with_extension("err");
    let (path, kernel) = (log.to_string_lossy().into_owned(), image.to_string_lossy());
    let (program, args) = command(host, &kernel, Some(&path), virtio, boot);

    let sink = fs::File::create(&errors)?;
    let mut child = Command::new(program).args(args).stderr(sink).spawn()?;
    thread::sleep(Duration::from_secs(seconds));
    let finished = child.try_wait()?;
    let _ = child.kill();
    let _ = child.wait();

    let console = fs::read_to_string(&log).unwrap_or_default();
    let why = fs::read_to_string(&errors).unwrap_or_default();
    let _ = (fs::remove_file(&log), fs::remove_file(&errors));
    match finished {
        Some(status) if console.is_empty() => Err(io::Error::other(format!(
            "{program} exited before it could boot ({status}): {}",
            why.trim()
        ))),
        _ => Ok(console),
    }
}

/// Boots under TCG, drives the shell, and writes the runtime layout it
/// printed, the events that led to it, and the trace derived from those
/// events: three artifacts from one boot. The document goes through the
/// same validator the static emitter uses.
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

/// Boots, headless or interactive, according to the arguments. `--run
/// SCRIPT` hands the shell a script at boot, the only way to ask a
/// headless guest anything.
pub fn boot(args: &[String]) -> io::Result<()> {
    let (host, seconds, debug) = crate::options(args, true)?;
    // `--console virtio` and `--run SCRIPT`; the parser in main.rs
    // accepts the pairs and the values are read back positionally here.
    let virtio = args.iter().any(|arg| arg == "virtio");
    // `mlsh.run=` goes last: it takes the rest of the string.
    let script = args.iter().skip_while(|arg| *arg != "--run").nth(1);
    let bootargs = script.map_or_else(String::new, |script| {
        format!("mlos.rev={} mlsh.run={script}", mlos_image_map::revision())
    });
    match seconds {
        Some(seconds) => {
            print!("{}", capture(host, seconds, virtio, &bootargs)?);
            Ok(())
        }
        None => run(host, debug, virtio, &bootargs),
    }
}
