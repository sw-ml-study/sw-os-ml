//! Boots MLOS and checks what it says.
//!
//! Under TCG, not HVF. TCG is slower and translates aarch64 to aarch64
//! for no gain in speed, but it is *deterministic*: a boot that fails
//! here fails the same way every run, which is the difference between a
//! kernel bug you can find and one you argue about. HVF is for using
//! MLOS; TCG is for testing it.
//!
//! Ignored by default. It builds a kernel and runs a VM, which is far too
//! slow for the ordinary `cargo test` gate and needs QEMU installed. CI
//! runs it explicitly:
//!
//! ```text
//! cargo test -p mlos-cli -- --ignored
//! ```

use std::process::Command;

/// The binary cargo built for this test.
const MLOS: &str = env!("CARGO_BIN_EXE_mlos");

/// How long to let the guest run. Generous: TCG on a loaded CI machine is
/// far slower than it is here, and a flaky timeout is worse than a slow
/// test.
const SECONDS: &str = "8";

/// Boots and returns everything the console said.
fn boot(extra: &[&str]) -> String {
    let mut args = vec!["run", "tcg", "--capture", SECONDS];
    args.extend_from_slice(extra);
    let out = Command::new(MLOS)
        .args(&args)
        .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
        .output()
        .expect("mlos runs");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Every line the kernel must produce, in order.
///
/// Order matters as much as presence: it is the boot sequence, and a
/// kernel that reports its memory map after enabling the MMU has done
/// something in the wrong order even if both lines appear.
const SEQUENCE: [&str; 6] = [
    "MLOS aarch64",
    "console",
    "usable",
    "Kernel",        // the image, carved out of the memory map
    "SCTLR_EL1.M=1", // translation actually on, read back from hardware
    "mlsh>",         // and it reached a shell
];

#[test]
#[ignore = "boots a VM; run with --ignored"]
fn it_boots_to_a_shell_over_the_pl011() {
    let console = boot(&[]);
    let mut rest = console.as_str();
    for expected in SEQUENCE {
        let found = rest.find(expected);
        assert!(found.is_some(), "missing {expected:?} in:\n{console}");
        rest = &rest[found.unwrap() + expected.len()..];
    }
    assert!(
        console.contains("console  pl011"),
        "should pick the PL011:\n{console}"
    );
}

/// The same boot, with the console on virtio instead.
///
/// Worth its own test because the two paths share nothing below the
/// banner: a virtqueue and a device handshake against a polled UART.
#[test]
#[ignore = "boots a VM; run with --ignored"]
fn it_boots_to_a_shell_over_virtio() {
    let console = boot(&["--console", "virtio"]);
    assert!(
        console.contains("console  virtio"),
        "should pick virtio:\n{console}"
    );
    assert!(
        console.contains("mlsh>"),
        "should reach the shell:\n{console}"
    );
}

/// The memory map must not offer the kernel's own memory.
///
/// This is the check that would have caught the gap step 006 left and
/// step 008 closed: `usable` counting the image is a number an allocator
/// would later trust.
#[test]
#[ignore = "boots a VM; run with --ignored"]
fn the_kernel_image_is_not_reported_as_usable() {
    let console = boot(&[]);
    assert!(
        console.contains("Kernel"),
        "the image should be marked:\n{console}"
    );
    assert!(
        console.contains("Reclaimable"),
        "the blob should be marked:\n{console}"
    );
    assert!(
        !console.contains("usable   512 MiB"),
        "512 would mean nothing was reserved"
    );
}
