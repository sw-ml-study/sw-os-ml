//! The x86-64 guest (saga `mlos-x86-64`): `--arch` routing, and boots.
//!
//! The routing tests are ordinary tests -- they start no VM. The boots are
//! ignored like `boot.rs`'s, and for the same reason: they build a kernel
//! and run QEMU. `cargo test -p mlos-cli -- --ignored` runs them.
//!
//! A separate file from `boot.rs` so the two architecture lanes never
//! edit the same test file.

use std::{
    env, fs,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

/// Numbers each boot's scratch directory. Tests run in parallel in one
/// process, so the process id alone is not unique: two boots sharing a
/// directory once deleted each other's console log mid-run.
static BOOTS: AtomicUsize = AtomicUsize::new(0);

/// The binary cargo built for this test.
const MLOS: &str = env!("CARGO_BIN_EXE_mlos");

/// The workspace root, where `target/` is.
const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

/// Runs `mlos` from the workspace root.
fn mlos(args: &[&str]) -> Output {
    let out = Command::new(MLOS).args(args).current_dir(ROOT).output();
    out.expect("mlos runs")
}

/// Boots the x86-64 kernel with `input` scripted onto COM1 and `append`
/// as its command line, straight through QEMU so the input can be a file.
/// A 1 MiB scratch disk is attached, so there is a virtio-mmio slot for
/// the command line to announce. Returns the exit status and everything
/// the console said.
///
/// The input is all there at t=0, before the kernel has touched the UART,
/// which is the case the driver's "leave the FIFO alone" exists for.
fn boot_with_input(cpu: &str, append: &str, input: &[u8]) -> (Option<i32>, String) {
    let built = mlos(&["--arch", "x86-64", "build"]);
    let elf = String::from_utf8_lossy(&built.stdout).trim().to_owned();
    let n = BOOTS.fetch_add(1, Ordering::Relaxed);
    let dir = env::temp_dir().join(format!("mlos-x86-{}-{n}", std::process::id()));
    fs::create_dir_all(&dir).expect("temp dir");
    let (inp, out, disk) = (dir.join("in"), dir.join("out"), dir.join("disk"));
    fs::write(&inp, input).expect("input written");
    fs::write(&disk, vec![0; 1 << 20]).expect("disk written");
    let com1 = format!(
        "file,id=com1,path={},input-path={}",
        out.display(),
        inp.display()
    );
    let drive = format!("if=none,id=d0,format=raw,file={}", disk.display());
    let status = Command::new("qemu-system-x86_64")
        .args([
            "-M",
            "microvm,acpi=off",
            "-accel",
            "tcg",
            "-cpu",
            cpu,
            "-m",
            "512",
        ])
        .args([
            "-display",
            "none",
            "-nodefaults",
            "-no-reboot",
            "-chardev",
            &com1,
        ])
        .args([
            "-serial",
            "chardev:com1",
            "-device",
            "isa-debug-exit,iobase=0xf4,iosize=0x04",
        ])
        .args(["-drive", &drive, "-device", "virtio-blk-device,drive=d0"])
        .args(["-append", append, "-kernel", &elf])
        .current_dir(ROOT)
        .status()
        .expect("qemu-system-x86_64 runs");
    let console = fs::read_to_string(&out)
        .unwrap_or_default()
        .replace('\r', "");
    let _ = fs::remove_dir_all(&dir);
    (status.code(), console)
}

#[test]
fn an_unknown_architecture_is_refused_by_name() {
    let out = mlos(&["--arch", "sparc", "build"]);
    assert!(!out.status.success());
    let why = String::from_utf8_lossy(&out.stderr);
    assert!(why.contains("\"sparc\"") && why.contains("x86-64"), "{why}");
}

#[test]
fn aarch64_takes_the_shared_path_and_hvf_is_refused_for_x86() {
    let out = mlos(&["--arch", "aarch64", "--version"]);
    assert!(out.status.success(), "{out:?}");
    let out = mlos(&["--arch", "x86-64", "run", "hvf"]);
    let why = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success() && why.contains("TCG-only"), "{why}");
}

/// Without `--arch`, `build` means this machine's architecture.
#[test]
#[ignore = "builds a kernel; run with --ignored"]
fn build_defaults_to_the_host_architecture() {
    let out = mlos(&["build"]);
    let path = String::from_utf8_lossy(&out.stdout);
    let expected = if cfg!(target_arch = "x86_64") {
        "x86_64-unknown-none"
    } else {
        "aarch64"
    };
    assert!(path.contains(expected), "{path}");
}

/// The banner is the first thing on the console, and `mlos run` shows it.
#[test]
#[ignore = "boots a VM; run with --ignored"]
fn the_banner_comes_first_and_names_the_architecture() {
    let out = mlos(&["--arch", "x86-64", "run", "tcg", "--capture", "20"]);
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{said}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let first = said.lines().find(|line| !line.trim().is_empty());
    assert_eq!(first.map(str::trim), Some("MLOS x86-64"), "{said}");
    for fact in [
        "16550 at 0x3f8",
        "long mode   yes",
        "start_info  valid",
        "1 GiB pages",
    ] {
        assert!(said.contains(fact), "missing {fact:?} in {said}");
    }
}

/// Receive: a command typed on COM1 reaches `mlsh` and runs, and
/// `Ctrl-D` ends the guest with reached | long mode | start_info | 1 GiB.
#[test]
#[ignore = "boots a VM; run with --ignored"]
fn a_typed_command_runs_in_the_shell_until_ctrl_d() {
    let (status, console) = boot_with_input("max", "", b"dev\r\x04");
    assert_eq!(status, Some((0x47 << 1) | 1), "{console}");
    assert!(
        console.contains("mlsh> dev\n  console  16550 @ 0x3f8, irq 4"),
        "{console}"
    );
}

/// `mem` and `dev` report what the loader and the command line said, not
/// what was assumed: the PVH map with the image carved out, the virtio
/// slot QEMU announced, a 16550, and no timer yet. And `mlsh.run=` runs
/// its verbs without QEMU's appended device list riding on the last one.
#[test]
#[ignore = "boots a VM; run with --ignored"]
fn mem_and_dev_report_what_pvh_and_the_command_line_said() {
    let (status, console) = boot_with_input("max", "mlsh.run=mem;dev", b"\x04");
    assert_eq!(status, Some((0x47 << 1) | 1), "{console}");
    for fact in [
        "memory map of",
        "virtio      1 slot(s) from the command line",
        " Kernel\n",
        "  image  0x100000 + ",
        "mlsh> dev\n",
        "  console  16550 @ 0x3f8, irq 4\n  timer    none\n  gic      none",
    ] {
        assert!(console.contains(fact), "missing {fact:?} in {console}");
    }
    assert!(
        !console.contains("Reclaimable"),
        "loader structures are in Reserved BIOS memory, left as is:\n{console}"
    );
}

/// QEMU's default CPU has no `pdpe1gb`, so this is the 2 MiB fallback:
/// the same exit bits minus 1 GiB pages.
#[test]
#[ignore = "boots a VM; run with --ignored"]
fn it_falls_back_to_two_megabyte_pages_without_pdpe1gb() {
    let (status, console) = boot_with_input("qemu64", "", b"\x04");
    assert_eq!(status, Some((0x43 << 1) | 1), "{console}");
    assert!(console.contains("2 MiB pages"), "{console}");
}

/// A page fault provoked from the shell and read back: `mem peek` on the
/// kernel's own first bytes answers (`cli` 0xfa, `cld` 0xfc), then on 1 GiB
/// -- just past the identity map -- the CPU takes #PF, and the report names
/// the vector, a not-present read (error 0), the faulting address in CR2,
/// and a RIP inside the kernel image. The guest then exits with the boot
/// bits plus "stopped by a trap" (0x08).
#[test]
#[ignore = "boots a VM; run with --ignored"]
fn a_page_fault_provoked_from_the_shell_is_reported() {
    let script = "mlsh.run=mem peek 0x100000;mem peek 0x40000000";
    let (status, console) = boot_with_input("max", script, b"");
    assert_eq!(status, Some((0x4f << 1) | 1), "{console}");
    assert!(
        console.contains("  0x100000: 0x") && console.contains("fcfa\n"),
        "{console}"
    );
    for fact in [
        "!! trap 14 (#PF page fault)",
        "   error  0x0000000000000000",
        "   cr2    0x0000000040000000",
    ] {
        assert!(console.contains(fact), "missing {fact:?} in {console}");
    }
    let rip = console
        .split("   rip    0x")
        .nth(1)
        .and_then(|r| u64::from_str_radix(&r[..16], 16).ok());
    assert!(
        rip.is_some_and(|rip| (0x10_0000..0x40_0000).contains(&rip)),
        "rip outside the image: {console}"
    );
}
