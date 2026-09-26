#!/usr/bin/env bash
# Build the skeleton and boot it; pass only if the banner appears and the
# guest exits through isa-debug-exit with the success code.
#
#   scripts/boot-test.sh [debug|release] [q35|pc|microvm]
#
# Tries KVM first and falls back to TCG, then reports which one ran --
# so the same command proves KVM on a Linux host and still works where
# /dev/kvm is absent (nested-virt-less CI and cloud containers).
set -euo pipefail
cd "$(dirname "$0")/.."

profile=${1:-debug}
machine=${2:-q35}
flag=(); [[ $profile == release ]] && flag=(--release)
cargo build -q "${flag[@]}"
image=target/x86_64-unknown-none/$profile/mlos-x86-skeleton

accel=(-accel tcg)
[[ -r /dev/kvm && -w /dev/kvm ]] && accel=(-accel kvm -cpu host)

log=$(mktemp)
set +e
timeout 60 qemu-system-x86_64 "${accel[@]}" -M "$machine" -m 128 \
    -display none -no-reboot -nodefaults -serial "file:$log" \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -kernel "$image"
status=$?
set -e

cat "$log"
want=("MLOS x86_64" "long mode: yes")
for line in "${want[@]}"; do
    grep -qF "$line" "$log" || { echo "FAIL: missing \"$line\" ($machine, $profile)"; exit 1; }
done
[[ $status == 33 ]] || { echo "FAIL: qemu exit $status, want 33 ($machine, $profile)"; exit 1; }
echo "PASS: $machine $profile under ${accel[1]}"
