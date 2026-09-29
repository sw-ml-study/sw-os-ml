//! Building the kernel and turning it into a bootable image.
//!
//! Invariant: a failed build stops here rather than reusing a stale
//! artifact. Design and history: docs/notes/mlos-cli.md.

use std::{io, path::PathBuf, process::Command};

/// Where the kernel and its flat image land.
pub const TARGET: &str = "aarch64-unknown-none-softfloat";

/// Builds the kernel and objcopies it to a flat arm64 `Image`. The
/// `Image`, not the ELF: QEMU boots an ELF by its entry point and skips
/// the arm64 boot protocol, so `x0` would not hold the device tree.
pub fn build() -> io::Result<PathBuf> {
    let elf = PathBuf::from("target")
        .join(TARGET)
        .join("debug/mlos-kernel");
    let image = elf.with_file_name("mlos.img");

    run(
        "cargo",
        &["build", "-q", "-p", "mlos-kernel", "--target", TARGET],
    )?;
    let objcopy = objcopy()?;
    run(
        &objcopy.to_string_lossy(),
        &[
            "-O",
            "binary",
            &elf.to_string_lossy(),
            &image.to_string_lossy(),
        ],
    )?;
    Ok(image)
}

/// Finds the `llvm-objcopy` that ships with the active toolchain, so it
/// matches the LLVM that produced the object files.
pub fn objcopy() -> io::Result<PathBuf> {
    let ask = |args: &[&str]| -> io::Result<String> {
        let out = Command::new("rustc").args(args).output()?;
        // Trimmed: `rustc --print sysroot` ends in a newline, and a path
        // with one in it silently does not exist.
        String::from_utf8(out.stdout)
            .map(|text| text.trim().to_owned())
            .map_err(io::Error::other)
    };
    let sysroot = ask(&["--print", "sysroot"])?;
    let host = ask(&["-vV"])?
        .lines()
        .find_map(|line| line.strip_prefix("host: ").map(str::to_owned))
        .ok_or_else(|| io::Error::other("rustc -vV did not report a host triple"))?;
    Ok(PathBuf::from(sysroot)
        .join("lib/rustlib")
        .join(host)
        .join("bin/llvm-objcopy"))
}

/// Runs a command, failing rather than continuing with a stale artifact.
fn run(program: &str, args: &[&str]) -> io::Result<()> {
    let status = Command::new(program).args(args).status()?;
    if status.success() {
        return Ok(());
    }
    Err(io::Error::other(format!("{program} failed: {status}")))
}

/// Writes a disk image holding the synthetic model's weights, then the
/// replay trace. Tile `(layer, tensor)` is at sector
/// `(layer * TILES + tensor) * TILE_BYTES / 512`, filled with
/// `0xA0 | (layer ^ tensor)`; the trace follows at
/// `mlos_synth::disk::TRACE_AT` as an eight-byte LE length then the text.
pub fn disk() -> io::Result<PathBuf> {
    use mlos_synth::{LAYERS, TILE_BYTES, TILES};

    let path = PathBuf::from("target").join(TARGET).join("debug/model.img");
    let bytes_per_tile = TILE_BYTES as usize;
    let mut bytes = Vec::with_capacity(usize::from(LAYERS) * usize::from(TILES) * bytes_per_tile);
    for layer in 0..LAYERS {
        for tensor in 0..TILES {
            let fill = 0xA0 | ((layer as u8) ^ (tensor as u8));
            bytes.extend(std::iter::repeat_n(fill, bytes_per_tile));
        }
    }
    let (sessions, rounds) = mlos_image_map::runtime::REPLAY;
    let trace = mlos_workload::Decode::of(sessions, rounds).text();
    bytes.extend((trace.len() as u64).to_le_bytes());
    bytes.extend(trace.as_bytes());
    bytes.resize(bytes.len().next_multiple_of(512), 0);

    std::fs::write(&path, &bytes)?;
    Ok(path)
}
