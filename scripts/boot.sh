#!/bin/sh
# Boot the kernel under QEMU and print what came out of the serial port.
#
#   scripts/boot.sh [tcg|hvf] [seconds]
#
# Superseded by `mlos run` in step 008; until then this is what steps
# 005-007 use to see whether a change still boots.
#
# Serial goes to a file and the monitor to stdio, not the other way round:
# with `-serial stdio` the "quit" lands on the guest's console instead of
# the monitor, and QEMU never exits.
set -eu

ACCEL=${1:-hvf}
SECONDS_TO_RUN=${2:-2}
KERNEL=target/aarch64-unknown-none-softfloat/debug/mlos-kernel
# `-u`: QEMU's file: chardev wants to create the file itself; handing it
# one mktemp already created yields an empty capture.
OUT=$(mktemp -u -t mlos-serial)

case "$ACCEL" in
    hvf) CPU=host ;;
    tcg) CPU=cortex-a72 ;;
    *)   echo "usage: $0 [tcg|hvf] [seconds]" >&2; exit 2 ;;
esac

[ -f "$KERNEL" ] || { echo "no kernel: run 'cargo kbuild-arm' first" >&2; exit 1; }

perl -e "select(undef,undef,undef,$SECONDS_TO_RUN); print \"quit\n\"" \
  | qemu-system-aarch64 \
        -M virt -cpu "$CPU" -accel "$ACCEL" -m 512 \
        -kernel "$KERNEL" \
        -display none -serial "file:$OUT" -monitor stdio \
        >/dev/null 2>&1 || true

cat "$OUT"
rm -f "$OUT"
