# mlos-gic-aarch64

A GICv3 interrupt controller driver: distributor, this CPU's
redistributor, and the CPU interface. Driver layer, below `mlos-hal`.
Linked from `crates/mlos-gic-aarch64/src/lib.rs`, `cpuif.rs`, `dist.rs`
and `redist.rs`.

## Why the crate is empty on other architectures

`#![cfg(target_arch = "aarch64")]` makes the crate compile to nothing
elsewhere, so the workspace-wide gate can sweep every crate without a
hand-maintained exclude list. The crate says where it applies; a list in
`.cargo/config.toml` would say it somewhere else and then drift, which is
what happened before the line existed.

## Only v3

A GICv2 is a different device with a different programming model, and
QEMU's `virt` will hand you either one depending on the accelerator: TCG
defaults to v2, HVF to v3. `scripts/boot.sh` pins `gic-version=3` rather
than letting the host decide what the guest is driving, and
`mlos-machine` checks the `compatible` string before believing the `reg`
ranges are a distributor and a redistributor.

## Three pieces, in an order that matters

The distributor routes system-wide, the redistributor holds this CPU's
private interrupts, and the CPU interface is system registers. Each will
accept writes before the one above it is ready, and ignore them. So
`Gic::new` brings them up in order: affinity routing before the
redistributor is addressable at all, the redistributor awake before its
registers mean anything, and the CPU interface last because it is what
starts delivery.

## Enabling dispatches by interrupt number

The two kinds live in different devices. Interrupts below 32 are private
to a CPU (PPIs and SGIs) and configured in its redistributor, in the SGI
frame one 64 KiB past the RD frame; everything above is shared and
configured in the distributor. Getting this the wrong way round writes to
a register that exists and does nothing. Looking for the timer's PPI in
the distributor is a common way to write a GICv3 driver that silently
never delivers anything.

Priority 0 is the most urgent. A priority must be numerically below
`ICC_PMR_EL1` or the CPU interface will never present it; `ACCEPT_ALL`
(`0xff`) in the PMR means the redistributor's per-interrupt priorities
are the only mask, so there is one place to look when an interrupt does
not arrive rather than two.

## The CPU interface

`ICC_SRE_EL1.SRE` comes first and needs its own `isb`: until it is set,
the other `ICC_*` registers are not architecturally accessible, and
writes to them are ignored rather than faulting, which looks exactly
like a controller that was configured and does nothing.

`acknowledge` returns the raw `ICC_IAR1_EL1` value; 1023 is the
architectural "spurious" answer, meaning nothing was pending after all.
Reading it moves the interrupt to active, which is the wanted side
effect. `end_of_interrupt` must pair with every acknowledge that returned
a real interrupt: skipping it leaves the interrupt active and the
controller will not deliver another at that priority, a hang that looks
like the timer stopped.

## The distributor

`GICD_CTLR.ARE_NS` is GICv3's defining feature and not optional: with it
clear the redistributors are not addressed at all and every per-CPU
interrupt is invisible. `enable` writes `ARE` first and Group 1 second,
because enabling delivery before routing exists would advertise
interrupts that have nowhere to go. Writes to `CTLR` are not
instantaneous; `RWP` is polled because proceeding while it is set means
configuring a controller that has not finished being configured.

`GICD_IROUTER` only exists from interrupt 32 up and only means anything
with affinity routing enabled. Shared interrupts are routed to affinity
0.0.0.0, the boot core, the only one running.

MMIO is accessed with volatile reads and writes because the device, not
the compiler, decides what a read means.

## The redistributor

`wake` clears `ProcessorSleep` and then waits for the redistributor to
acknowledge by clearing `ChildrenAsleep`. Configuring it before it has
woken is configuring nothing.
