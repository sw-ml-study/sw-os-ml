`mlos-trap-x86-64`: IDT, exception entry, a fault report naming vector, error code, `RIP` and `CR2` -- the same shape the aarch64 report gives for `ESR`/`ELR`/`FAR`. Provoked on purpose from the shell and read back.

From docs/plan.md saga `mlos-x86-64`. Not in this saga: SMP, PCI, ACPI beyond the PM timer, UEFI, KVM.
