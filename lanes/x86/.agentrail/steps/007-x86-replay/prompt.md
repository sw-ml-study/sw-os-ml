`model 32; replay demand|fifo|lru|next-use` under x86-64 TCG, and a boot test asserting each line equals the aarch64 guest's and the simulator's, exactly. `docs/status.md` gains the architecture as a column. This is the step the saga exists for.

From docs/plan.md saga `mlos-x86-64`. Not in this saga: SMP, PCI, ACPI beyond the PM timer, UEFI, KVM.
