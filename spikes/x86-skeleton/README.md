# x86-64 skeleton spike

**Not part of the MLOS build.** A standalone Cargo project (its own
`[workspace]`) that proves, before the x86-64 lane starts, that the
toolchain, image format, emulator and test loop all work for an x86-64
MLOS kernel written in Rust. The real work goes into `crates/`; this
directory is a reference and can be deleted once it has been absorbed.

## Run it

```bash
scripts/boot-test.sh                  # debug, q35
scripts/boot-test.sh release microvm  # any of debug|release x q35|pc|microvm
```

Needs `qemu-system-x86_64` (Ubuntu: `apt-get install qemu-system-x86`,
*with* recommends -- the firmware ROMs are in them). The toolchain is
pinned to the same 1.96.0 as the repo root.

Pass means: the banner appears on COM1 **and** the guest ends through
`isa-debug-exit` with status 33 (`(0x10 << 1) | 1`). A guest that hangs,
triple-faults or never reaches long mode fails the script.

```
MLOS x86_64
long mode: yes
pvh start_info: 0x21e0
hypervisor: TCG
PASS: q35 debug under tcg
```

## KVM

The script uses KVM when `/dev/kvm` is readable and writable, TCG
otherwise, and says which ran; the guest independently reports the
hypervisor it sees through CPUID leaf `0x40000000`. On a Linux host with
KVM, expect `hypervisor: KVM` and `PASS: ... under kvm`.

Verified so far under **TCG only** (QEMU 8.2.2, Ubuntu 24.04), on all six
of debug/release x q35/pc/microvm. The cloud container it was written in
is itself a VM without nested virtualization (no `vmx`/`svm`, no
`/dev/kvm`), so KVM is **not yet verified** -- run the script on a Linux
host to close that.

## Design decisions

- **Boot protocol: PVH.** QEMU loads the 64-bit ELF directly with
  `-kernel` when it carries a `XEN_ELFNOTE_PHYS32_ENTRY` note, entering
  in 32-bit protected mode with paging off. No firmware, no bootloader,
  no image conversion -- the x86 counterpart of the arm64 `Image` header
  the aarch64 kernel uses. Firecracker and Cloud Hypervisor boot PVH
  kernels too. Multiboot was ruled out: QEMU's multiboot loader refuses
  ELF64. UEFI was ruled out for now for the same reason the aarch64 side
  parked `017-efi-stub`: nothing yet needs firmware services.
- **Entry: 32-bit asm, then Rust.** `src/boot.s` identity-maps the first
  1 GiB with 2 MiB pages, enables PAE, sets EFER.LME, turns on paging,
  loads a three-entry GDT and far-jumps to 64-bit code, which calls
  `kmain`. Everything after that is Rust.
- **Test exit: `isa-debug-exit`** at port `0xf4`, so the host gets a
  real status code instead of scraping a console for a timeout.

## What the real x86-64 lane must carry over

1. **`linker/x86_64.ld` discards `.note*`.** That drops the PVH note and
   QEMU no longer finds the entry point. It must `KEEP(*(.note.pvh))`
   (see `linker.ld` here).
2. **Static relocation model.** `x86_64-unknown-none` defaults to PIE;
   the 32-bit entry needs absolute addresses before paging exists. Here
   it is `-C relocation-model=static` in `.cargo/config.toml` under
   `[target.x86_64-unknown-none]`.
3. **LLVM Intel-syntax trap.** `push offset sym` in `.code32` is encoded
   with a 16-bit immediate and fails to link for any address above
   64 KiB (`R_X86_64_16 out of range`). The far jump goes through a
   memory far pointer instead (`jmp fword ptr [long_mode_ptr]`).
4. **`mlos-cli` is aarch64-only today** (`image::TARGET`,
   `qemu-system-aarch64`, `hvf|tcg|vz`, the `"MLOS aarch64"` boot-test
   string). x86 needs a target, `qemu-system-x86_64`, `-accel kvm|tcg`
   and the debug-exit device.
5. **Cargo inherits the root `.cargo/config.toml`.** Its shared
   `target-dir` applies to anything under the repo, so this spike pins
   its own `target-dir`; without that the image lands in the root
   `target/` and the script boots nothing.

## Gate status

`cargo fmt --check` and `cargo clippy -- -D warnings` clean.
`sw-checklist`: 0 failed, 1 warning -- `port.rs` holds 6 functions.
Accepted for a throwaway spike; in the repo, port I/O belongs in an
x86-64 HAL crate split to the <=4-function gate.
