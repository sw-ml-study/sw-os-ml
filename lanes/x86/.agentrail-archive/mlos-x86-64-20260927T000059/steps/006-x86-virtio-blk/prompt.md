the existing `mlos-virtio-blk` over virtio-mmio on `microvm`, the model disk attached, `model` reporting `weights from virtio-blk`, and the `0xA0` provenance nibble read back. Zero new driver code is the expected result; a line of it is a finding.

From docs/plan.md saga `mlos-x86-64`. Not in this saga: SMP, PCI, ACPI beyond the PM timer, UEFI, KVM.
