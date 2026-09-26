`mlos-apic-x86-64`: LAPIC and IOAPIC, the LAPIC timer at the 2 Hz tick the shell already counts, the serial receive interrupt, and a nanosecond clock for `sweep` from the TSC with its rate read from `CPUID.15H` where QEMU offers it and calibrated against the ACPI PM timer where it does not. Which of the two it was is printed, because a calibrated clock is not a read one.

From docs/plan.md saga `mlos-x86-64`. Not in this saga: SMP, PCI, ACPI beyond the PM timer, UEFI, KVM.
