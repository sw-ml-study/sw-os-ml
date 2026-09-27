`mlos-hal-x86-64`: PVH note, 32-bit entry, long mode, 1 GiB identity map, stack, `.bss` cleared, into `mlos_main` with the `hvm_start_info` pointer. `mlos build --arch x86-64` and `mlos run --arch x86-64 tcg` in `mlos-cli`, both architectures selectable and neither the default of the other. A `hlt` is no longer the entry.

From docs/plan.md saga `mlos-x86-64`. Not in this saga: SMP, PCI, ACPI beyond the PM timer, UEFI, KVM.
