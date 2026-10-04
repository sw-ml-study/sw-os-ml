# K on MLOS ARM64

This is the working path for running the portable K core from `kbm-fork` as
an in-kernel MLOS command on Apple Silicon. The guest is AArch64 throughout:
QEMU uses the host ARM64 CPU through HVF by default. The x86-64 kbm/BareMetal
image is not involved.

## Prerequisites

On macOS, verify the MLOS toolchain and VM tools:

```sh
cd /Users/mike/github/sw-ml-study/sw-os-ml
cargo run -q -p mlos-cli -- doctor
```

The important checks are `qemu-system-aarch64`, `hvf`, the
`aarch64-unknown-none-softfloat` Rust target, and `llvm-objcopy`.

## Build and boot

The one-command launcher builds the kernel and enters K at boot:

```sh
cd /Users/mike/github/sw-ml-study/sw-os-ml
./scripts/arm64-k-repl.sh
```

The launcher is equivalent to:

```sh
cargo run -q -p mlos-cli -- run --run k
```

The default VM command is `qemu-system-aarch64 -cpu host -accel hvf`, so the
guest executes the Apple Silicon instruction set with hardware virtualization.
For deterministic software emulation instead:

```sh
cargo run -q -p mlos-cli -- run tcg --run k
```

## Use the REPL

At the K prompt (`$`), enter an expression and press Return:

```text
1+1
2
```

K input is supplied by MLOS's interrupt-driven PL011 console queue. The bridge
normalizes macOS/QEMU carriage-return input to K's newline terminator. K output
uses the same MLOS console. Press `Ctrl-A x` to exit QEMU.

## What is built where

The portable K sources are vendored under `ksrc/` from the
`kbm-fork` `origin/spike/portable-k` line. `crates/mlos-k-ffi/build.rs`
compiles `a.c` and `z.c` for `aarch64-none-elf`, with `-DKSYS` and
`-DKHEAP=22`, and links the resulting archive into `mlos-kernel`.

The runtime boundary is split as follows:

- `crates/mlos-k-ffi/`: C build, fixed-arity `k_sys`, and AArch64 FP/SIMD
  enable/disable around K.
- `crates/mlos-kernel/src/kbridge.rs`: MLOS console and input callbacks.
- `crates/mlsh/src/commands.rs`: the `k` shell command.
- `crates/mlos-cli/src/run.rs`: preserves `--run k` for interactive boots.

`KHEAP=22` gives K a 256 MiB heap so the image fits MLOS's current 512 MiB
guest. The later integration step should replace the current return-on-exit
behavior with a trampoline that returns K's `\\` command directly to `mlsh`.

## Verification

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo kclippy-arm -- -D warnings
cargo kbuild-arm
cargo test
```

The interactive acceptance check is simply to boot with the launcher and
evaluate `1+1` at the K prompt under both default HVF and `tcg`.
