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
};

/// The binary cargo built for this test.
const MLOS: &str = env!("CARGO_BIN_EXE_mlos");

/// The workspace root, where `target/` is.
const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

/// Runs `mlos` from the workspace root.
fn mlos(args: &[&str]) -> Output {
    let out = Command::new(MLOS).args(args).current_dir(ROOT).output();
    out.expect("mlos runs")
}

/// Boots the x86-64 kernel with `input` scripted onto COM1, straight
/// through QEMU so the input can be a file. Returns the exit status and
/// everything the console said.
///
/// The input is all there at t=0, before the kernel has touched the UART,
/// which is the case the driver's "leave the FIFO alone" exists for.
fn boot_with_input(cpu: &str, input: &[u8]) -> (Option<i32>, String) {
    let built = mlos(&["--arch", "x86-64", "build"]);
    let elf = String::from_utf8_lossy(&built.stdout).trim().to_owned();
    let dir = env::temp_dir().join(format!("mlos-x86-{}-{cpu}", std::process::id()));
    fs::create_dir_all(&dir).expect("temp dir");
    let (inp, out) = (dir.join("in"), dir.join("out"));
    fs::write(&inp, input).expect("input written");
    let com1 = format!(
        "file,id=com1,path={},input-path={}",
        out.display(),
        inp.display()
    );
    let status = Command::new("qemu-system-x86_64")
        .args(["-M", "microvm", "-accel", "tcg", "-cpu", cpu, "-m", "512"])
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
        .args(["-kernel", &elf])
        .current_dir(ROOT)
        .status()
        .expect("qemu-system-x86_64 runs");
    let console = fs::read_to_string(&out).unwrap_or_default();
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

/// Receive: what is typed comes back, and `Ctrl-D` ends the guest with
/// reached | long mode | start_info | 1 GiB pages.
#[test]
#[ignore = "boots a VM; run with --ignored"]
fn it_echoes_what_it_receives_until_ctrl_d() {
    let (status, console) = boot_with_input("max", b"hello, 16550\rsecond line\n\x04");
    assert_eq!(status, Some((0x47 << 1) | 1), "{console}");
    let echoed = console.split("Ctrl-D ends").nth(1).unwrap_or_default();
    assert_eq!(echoed.trim(), "hello, 16550\r\nsecond line", "{console}");
}

/// QEMU's default CPU has no `pdpe1gb`, so this is the 2 MiB fallback:
/// the same exit bits minus 1 GiB pages.
#[test]
#[ignore = "boots a VM; run with --ignored"]
fn it_falls_back_to_two_megabyte_pages_without_pdpe1gb() {
    let (status, console) = boot_with_input("qemu64", b"\x04");
    assert_eq!(status, Some((0x43 << 1) | 1), "{console}");
    assert!(console.contains("2 MiB pages"), "{console}");
}
