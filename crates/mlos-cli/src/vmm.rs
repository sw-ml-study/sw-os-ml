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
pub fn command(
    host: &str,
    image: &str,
    log: Option<&str>,
    virtio: bool,
) -> (&'static str, Vec<String>) {
    if host == "vz" {
        return ("vfkit", vfkit(image, log));
    }
    let cpu = if host == "hvf" { "host" } else { "cortex-a72" };
    let args = if virtio {
        qemu_virtio(cpu, host, image, log)
    } else {
        qemu(cpu, host, image, log)
    };
    ("qemu-system-aarch64", args)
}

/// QEMU with the console on the PL011.
fn qemu(cpu: &str, host: &str, image: &str, log: Option<&str>) -> Vec<String> {
    let owned = |args: &[&str]| args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
    let mut args = owned(&["-M", "virt,gic-version=3", "-cpu", cpu, "-accel", host]);
    args.extend(owned(&["-m", "512", "-display", "none", "-kernel", image]));
    args.extend(disk());
    match log {
        Some(log) => args.extend(owned(&[
            "-serial",
            &format!("file:{log}"),
            "-monitor",
            "none",
        ])),
        None => args.extend(owned(&["-serial", "mon:stdio"])),
    }
    args
}

/// QEMU with the console on virtio rather than the PL011.
///
/// `force-legacy=false` is required, not cosmetic: QEMU's `virt` defaults
/// virtio-mmio to version 1, the legacy layout with a different queue
/// convention, and MLOS speaks only version 2. Without it the magic value
/// matches, the version does not, and every slot probes as absent.
///
/// `console=hvc0` is what tells MLOS to prefer it -- the same `console=`
/// convention Linux uses.
fn qemu_virtio(cpu: &str, host: &str, image: &str, log: Option<&str>) -> Vec<String> {
    let owned = |args: &[&str]| args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
    let mut args = owned(&["-M", "virt,gic-version=3", "-cpu", cpu, "-accel", host]);
    args.extend(owned(&["-m", "512", "-display", "none", "-kernel", image]));
    args.extend(disk());
    args.extend(owned(&["-global", "virtio-mmio.force-legacy=false"]));
    args.extend(owned(&[
        "-append",
        "console=hvc0",
        "-serial",
        "none",
        "-monitor",
        "none",
    ]));
    args.extend(owned(&["-device", "virtio-serial-device,id=vs0"]));
    let chardev = log.map_or_else(
        || "stdio,id=c0".to_owned(),
        |log| format!("file,id=c0,path={log}"),
    );
    args.extend(["-chardev".to_owned(), chardev]);
    args.extend(owned(&["-device", "virtconsole,chardev=c0,bus=vs0.0"]));
    args
}

/// vfkit's arguments: Apple's Virtualization.framework, via
/// `VZLinuxBootLoader`, which takes the raw arm64 image directly.
fn vfkit(image: &str, log: Option<&str>) -> Vec<String> {
    let serial = log.map_or_else(
        || "virtio-serial,stdio".to_owned(),
        |log| format!("virtio-serial,logFilePath={log}"),
    );
    // vfkit insists on an initrd even though `VZLinuxBootLoader` does
    // not, so it gets a single zero byte. MLOS never looks at it.
    let initrd = std::env::temp_dir().join("mlos-empty.initrd");
    let _ = fs::write(&initrd, [0u8]);
    let boot = format!("linux,kernel={image},initrd={}", initrd.display());
    let mut args = ["--cpus", "2", "--memory", "512", "--bootloader"]
        .iter()
        .map(|arg| (*arg).to_owned())
        .collect::<Vec<_>>();
    args.extend([boot, "--device".to_owned(), serial]);
    args
}

/// The model disk, attached read-only.
///
/// `force-legacy=false` again, and for the same reason as the console:
/// QEMU defaults virtio-mmio to version 1, MLOS speaks only version 2, and
/// without it the device is present and probes as absent.
///
/// Read-only because weights are immutable, which is the property that
/// lets one copy serve every session. A writable model disk would be a
/// tier that has to be invalidated.
fn disk() -> Vec<String> {
    let Ok(path) = crate::image::disk() else {
        return Vec::new();
    };
    vec![
        "-global".to_owned(),
        "virtio-mmio.force-legacy=false".to_owned(),
        "-drive".to_owned(),
        format!(
            "file={},format=raw,if=none,id=model,readonly=on",
            path.display()
        ),
        "-device".to_owned(),
        "virtio-blk-device,drive=model".to_owned(),
    ]
}
