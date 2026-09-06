//! Turning "which hypervisor" into a command line.
//!
//! Its own module because QEMU and vfkit are not variants of one thing.
//! They are separate programs with separate argument conventions, and the
//! only reason `mlos run` treats them alike is that from the outside they
//! answer the same question: what runs MLOS.

use std::fs;

/// The program and its whole command line, console included.
///
/// `gic-version=3` is pinned for QEMU rather than left to it: the default
/// differs by accelerator -- TCG gives a GICv2, HVF a v3 -- so without it
/// the two hand the guest different interrupt controllers, and only one is
/// the one MLOS drives.
///
/// `-serial mon:stdio` multiplexes the guest console with the QEMU
/// monitor, which is what makes `Ctrl-A x` work. vfkit has no monitor.
pub fn command(host: &str, image: &str, log: Option<&str>) -> (&'static str, Vec<String>) {
    let owned = |args: &[&str]| args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
    if host == "vz" {
        return ("vfkit", vfkit(image, log));
    }
    let cpu = if host == "hvf" { "host" } else { "cortex-a72" };
    let mut args = owned(&["-M", "virt,gic-version=3", "-cpu", cpu, "-accel", host]);
    args.extend(owned(&["-m", "512", "-display", "none", "-kernel", image]));
    match log {
        Some(log) => args.extend(owned(&[
            "-serial",
            &format!("file:{log}"),
            "-monitor",
            "none",
        ])),
        None => args.extend(owned(&["-serial", "mon:stdio"])),
    }
    ("qemu-system-aarch64", args)
}

/// vfkit's arguments: Apple's Virtualization.framework, via
/// `VZLinuxBootLoader`, which takes the raw arm64 image directly.
fn vfkit(image: &str, log: Option<&str>) -> Vec<String> {
    let serial = log.map_or_else(
        || "virtio-serial,stdio".to_owned(),
        |log| format!("virtio-serial,logFilePath={log}"),
    );
    let boot = format!("linux,kernel={image},initrd={}", empty_initrd().display());
    let mut args = ["--cpus", "2", "--memory", "512", "--bootloader"]
        .iter()
        .map(|arg| (*arg).to_owned())
        .collect::<Vec<_>>();
    args.extend([boot, "--device".to_owned(), serial]);
    args
}

/// An initrd vfkit will accept.
///
/// It insists on one even though `VZLinuxBootLoader` does not, so it gets
/// a single zero byte. MLOS never looks at it.
fn empty_initrd() -> std::path::PathBuf {
    let path = std::env::temp_dir().join("mlos-empty.initrd");
    let _ = fs::write(&path, [0u8]);
    path
}
