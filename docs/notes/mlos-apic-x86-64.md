# mlos-apic-x86-64

The x86-64 interrupt controllers and clocks: the local APIC and its timer,
the IOAPIC, the legacy 8259 PIC (silenced), and the TSC as a clock.
Driver layer, the counterpart of `mlos-gic-aarch64` plus the generic
timer. Linked from `crates/mlos-apic-x86-64/src/lib.rs`, `clock.rs`,
`lapic.rs` and `timer.rs`.

## Why its own crate

Register access to the LAPIC and IOAPIC, port I/O to the PIC and PIT, and
the `IA32_APIC_BASE` MSR are all `unsafe`, and `unsafe` is confined to
driver crates. The kernel's `handlers.rs` says what each vector does; this
crate says how the controllers are found, silenced, routed and
acknowledged.

## Discovery without ACPI

On `microvm` with ACPI off nothing describes these devices: no device
tree, no MADT. So the LAPIC base is discovered, read from
`IA32_APIC_BASE` rather than assumed, and the IOAPIC is taken to be at
`IOAPIC_BASE` (`0xfec00000`), the PC convention that only ACPI's MADT
could contradict. Both lie in the 3-4 GiB device window that
`mlos-hal-x86-64`'s entry maps uncached; the LAPIC lies there only
because `0xfee00000` is what every PC reports, so a base outside the
window is refused rather than dereferenced through a mapping that does
not exist.

The LAPIC's registers are 32-bit and must be accessed as such; `read` and
`write` are volatile so the compiler neither merges nor elides them.

## Silencing the PIC

The 8259s are put through their initialisation sequence, remapped away
from the exception vectors to `0xe0` and `0xe8`, and then every line is
masked. They are parked at vectors nothing else uses so that a spurious
PIC interrupt could never be mistaken for a real one. From then on the
IOAPIC delivers everything.

## Routing ISA lines

`route` programs one IOAPIC redirection entry: fixed delivery, physical
destination, edge-triggered, active high, which is what an ISA line is.
On `microvm` ISA IRQ `n` is IOAPIC pin `n`, with no interrupt source
override, so the pin and the IRQ are the same number. Both halves of the
64-bit entry are written through `IOREGSEL`/`IOWIN`, and as 32-bit
accesses, as the IOAPIC requires.

## The TSC as a clock

`CPUID.15H` gives the TSC's ratio to a crystal whose frequency it also
gives, when the CPU reports all three fields; then the rate is read.
Otherwise it is calibrated: counted across a known interval of the 8254
PIT, which runs at 1.193182 MHz on every PC. Which it was is reported in
the banner, because a calibrated clock is not a read one and someone
comparing timings should know which they have.

`docs/plan.md` named the ACPI PM timer as the calibration reference, but
`microvm` runs with ACPI off (the virtio slots appear on the command line
only then) and so has no PM timer. The PIT is there regardless.

`CALIBRATION_MS` is 50: long enough that one PIT tick of slop is under
0.002%, short enough not to be noticed at boot. `wait_ms` free-runs PIT
channel 0 in mode 2 from 65536 and latches and reads the count until the
elapsed ticks cover the interval. No interrupt is involved: IRQ 0 is
masked at the PIC and not routed at the IOAPIC, so the PIT can be used as
a stopwatch without firing anything.

`mlsh`'s clock takes a `u32` rate, and a TSC above 4.29 GHz does not fit
one. `tsc_rate` picks a right shift that brings the rate under `u32::MAX`
and `now` applies the same shift to the counter, so the two agree.

## The LAPIC timer

`timer_rate` measures the timer's own count rate (after the divide by
16) against `wait_ms`, with the LVT entry masked so the timer counts but
raises nothing, then hands the number back for `start_timer` to divide
into the wanted frequency. The rate is measured, not assumed, for the
same reason the TSC's is.

Every vector but the spurious one must end with an EOI, or the LAPIC
delivers nothing lower again. The spurious vector (`0xff`) needs none.
