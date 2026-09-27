memory map from the PVH table, virtio-mmio slots parsed from the command line, `mlsh.run=` honoured from the same string. `mem` and `dev` in the shell report what was found rather than what was assumed. Nothing from `mlos-fdt` is linked.

From docs/plan.md saga `mlos-x86-64`. Not in this saga: SMP, PCI, ACPI beyond the PM timer, UEFI, KVM.
