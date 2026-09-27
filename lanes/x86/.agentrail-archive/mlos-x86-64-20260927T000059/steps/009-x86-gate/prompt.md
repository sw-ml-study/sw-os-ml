`kbuild-x86` and `kclippy-x86` already run; the boot tests now run both architectures whenever both QEMU binaries exist, and `mlos doctor` says which are missing. `sw-checklist` at the same count it started at.

From docs/plan.md saga `mlos-x86-64`. Not in this saga: SMP, PCI, ACPI beyond the PM timer, UEFI, KVM.
