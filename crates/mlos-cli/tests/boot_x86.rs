//! The x86-64 guest (saga `mlos-x86-64`): `--arch` routing, and a boot.
//!
//! The routing tests are ordinary tests -- they start no VM. The boots are
//! ignored like `boot.rs`'s, and for the same reason: they build a kernel
//! and run QEMU. `cargo test -p mlos-cli -- --ignored` runs them.
//!
//! A separate file from `boot.rs` so the two architecture lanes never
//! edit the same test file.

use std::process::{Command, Output};

/// The binary cargo built for this test.
const MLOS: &str = env!("CARGO_BIN_EXE_mlos");

/// Runs `mlos` from the workspace root, where `target/` is.
fn mlos(args: &[&str]) -> Output {
    Command::new(MLOS)
        .args(args)
        .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
        .output()
        .expect("mlos runs")
}

#[test]
fn an_unknown_architecture_is_refused_by_name() {
    let out = mlos(&["--arch", "sparc", "build"]);
    assert!(!out.status.success());
    let why = String::from_utf8_lossy(&out.stderr);
    assert!(why.contains("\"sparc\"") && why.contains("x86-64"), "{why}");
}

#[test]
fn aarch64_takes_the_unchanged_path_and_hvf_is_refused_for_x86() {
    let out = mlos(&["--arch", "aarch64", "--version"]);
    assert!(out.status.success(), "{out:?}");
    let out = mlos(&["--arch", "x86-64", "run", "hvf"]);
    let why = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success() && why.contains("TCG-only"), "{why}");
}

#[test]
#[ignore = "boots a VM; run with --ignored"]
fn it_reaches_mlos_main_in_long_mode_with_gigabyte_pages() {
    let out = mlos(&["--arch", "x86-64", "run", "tcg", "--capture", "30"]);
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{said}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    for fact in [
        "reached mlos_main",
        "long mode yes",
        "start_info valid",
        "1 GiB pages",
    ] {
        assert!(said.contains(fact), "missing {fact:?} in {said}");
    }
}

/// QEMU's default CPU has no `pdpe1gb`, so this is the 2 MiB fallback.
/// Straight to QEMU: `mlos run` always asks for `-cpu max`.
#[test]
#[ignore = "boots a VM; run with --ignored"]
fn it_falls_back_to_two_megabyte_pages_without_pdpe1gb() {
    let built = mlos(&["--arch", "x86-64", "build"]);
    let elf = String::from_utf8_lossy(&built.stdout).trim().to_owned();
    let status = Command::new("qemu-system-x86_64")
        .args([
            "-M", "microvm", "-accel", "tcg", "-cpu", "qemu64", "-m", "512",
        ])
        .args(["-display", "none", "-nodefaults", "-no-reboot"])
        .args([
            "-device",
            "isa-debug-exit,iobase=0xf4,iosize=0x04",
            "-kernel",
            &elf,
        ])
        .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
        .status()
        .expect("qemu-system-x86_64 runs");
    // reached | long mode | start_info, and NOT the 1 GiB bit.
    assert_eq!(status.code(), Some((0x43 << 1) | 1), "{status:?}");
}
