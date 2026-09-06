#!/bin/sh
# Build the kernel and objcopy it to a flat arm64 Image.
#
# Separate from boot.sh because more than one thing needs the image now
# (boot.sh, demos/*.tape), and a stale image is a silent failure: QEMU
# happily boots yesterday's kernel and the output looks almost right.
# `mlos build` takes this over in step 013.
set -eu

KERNEL=target/aarch64-unknown-none-softfloat/debug/mlos-kernel
IMAGE=target/aarch64-unknown-none-softfloat/debug/mlos.img

cargo kbuild-arm -q
HOST=$(rustc -vV | sed -n 's/^host: //p')
"$(rustc --print sysroot)/lib/rustlib/$HOST/bin/llvm-objcopy" -O binary "$KERNEL" "$IMAGE"
echo "$IMAGE"
