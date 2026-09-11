//! Building the kernel and turning it into a bootable image.

use std::{io, path::PathBuf, process::Command};

/// Where the kernel and its flat image land.
pub const TARGET: &str = "aarch64-unknown-none-softfloat";

/// Builds the kernel and objcopies it to a flat arm64 `Image`.
///
/// The `Image`, not the ELF. QEMU jumps straight to an ELF's entry point
/// and skips the arm64 boot protocol, so `x0` arrives as zero instead of a
/// device tree pointer -- which is how MLOS spent step 004 not knowing
/// where its own device tree was.
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

/// Finds the `llvm-objcopy` that ships with the active toolchain.
///
/// Rather than requiring one on `PATH`: the toolchain's own is guaranteed
/// to match the LLVM that produced the object files, and a mismatched
/// system objcopy fails in ways that look like a linker bug.
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

/// Runs a command, failing loudly rather than continuing with a stale
/// artifact -- a stale image boots happily and looks almost right.
fn run(program: &str, args: &[&str]) -> io::Result<()> {
    let status = Command::new(program).args(args).status()?;
    if status.success() {
        return Ok(());
    }
    Err(io::Error::other(format!("{program} failed: {status}")))
}

/// Writes a disk image holding the synthetic model's weights.
///
/// Laid out by id: tile `(layer, tensor)` at sector
/// `(layer * TILES + tensor) * TILE_BYTES / 512`, filled with
/// `layer ^ tensor`. The same pattern the stub provider fabricates, on
/// purpose -- so when the guest reads it back from a real device, the
/// bytes being *right* is not the news. The news is where they came from.
pub fn disk() -> io::Result<PathBuf> {
    use mlos_synth::{LAYERS, TILE_BYTES, TILES};

    let path = PathBuf::from("target").join(TARGET).join("debug/model.img");
    let bytes_per_tile = TILE_BYTES as usize;
    let mut bytes = Vec::with_capacity(usize::from(LAYERS) * usize::from(TILES) * bytes_per_tile);
    for layer in 0..LAYERS {
        for tensor in 0..TILES {
            // 0xA0 | (layer ^ tensor). The high nibble is the point: the
            // stub provider fills with `layer ^ tensor` alone, so a byte
            // with 0xA0 in it can only have come off the disk. "The right
            // bytes arrived" is then a statement about provenance rather
            // than about arithmetic.
            let fill = 0xA0 | ((layer as u8) ^ (tensor as u8));
            bytes.extend(std::iter::repeat_n(fill, bytes_per_tile));
        }
    }
    std::fs::write(&path, &bytes)?;
    Ok(path)
}
