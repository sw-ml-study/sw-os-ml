//! Supplies the kernel image layout to the linker.

use std::{env, path::PathBuf};

/// Points `rust-lld` at the linker script for the target architecture.
///
/// The path is made absolute: a relative `-T` is resolved against the
/// linker's working directory, which cargo does not guarantee.
fn main() {
    let arch = env::var("CARGO_CFG_TARGET_ARCH").expect("cargo sets target arch");
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../linker")
        .join(format!("{arch}.ld"));

    println!("cargo::rustc-link-arg=-T{}", script.display());
    println!("cargo::rerun-if-changed={}", script.display());
}
