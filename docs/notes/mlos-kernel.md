# mlos-kernel

The microkernel binary: the entry point, bring-up, the boot banner, and
the interrupt handlers. Kernel layer, on top of `mlos-hal-aarch64`,
`mlos-mmu-aarch64`, `mlos-trap-aarch64`, `mlos-gic-aarch64` and
`mlos-machine`. Linked from `crates/mlos-kernel/src/main.rs`, `boot.rs`,
`banner.rs` and `handlers.rs`.

## What belongs here

Milestone M1 is bring-up (`docs/plan.md`); nothing ML-shaped belongs in
the kernel until it boots. The object table is M2, and resisting it until
then was the point.

## Why `boot` and `banner` are their own modules

The entry point should read as a single decision: run bring-up, and if
it declines, stop. `boot` holds the sequence so `mlos_main` stays that
one decision; `banner` holds the printing so the sequence reads as
decisions rather than as print statements with probes attached.

## The console is discovered, not assumed

Step 004 hardcoded the QEMU `virt` PL011 base as an explicit crutch;
step 005 deleted it once the `Image` header delivered the device tree
pointer in `x0`. The consequence is deliberate: a machine whose device
tree we cannot read is a machine we cannot run on, so a failed probe
parks silently rather than limping on a guessed address. Limping along
would hide the failure instead of showing it. Diagnosing a silent park is
what the gdb stub is for, and is one reason QEMU is the development
target (`docs/architecture.md` s.8.1).

## x86-64 lives in its own crate

On x86-64 the entry is `mlos-hal-x86-64`'s PVH `_start`, and `mlos_main`
is in `mlos-kernel-x86-64`, a crate of its own so the two architectures
never edit the same file. `main.rs` links it for its `#[no_mangle]`
symbol and nothing else.

## Bring-up order

`arm_interrupts` installs the vector table, brings up the interrupt
controller, starts the timer, and unmasks last. Order is load-bearing
throughout, and each step is quiet when wrong: vectors before anything
can trap, the controller before an interrupt has anywhere to go, the
timer before it is unmasked, and the unmask last. Unmasking early with a
half-built controller behind it fires immediately and repeatedly, which
is far harder to read than a machine that simply never ticks.

The timer ticks twice a second: slow enough to read on a console, fast
enough that a boot capture of a few seconds shows time actually passing.

The virtio slot window is handed to `mlos_lab` from `facts` because that
is where the shell's dependencies are gathered, and because the device
tree is the kernel's to read: nothing above it should be parsing one.

## Banner lines prove the chain

`report` prints what the device tree said, so a boot that reaches it
proves the whole chain: `Image` header, `x0`, the tree walk, and the
console address discovered from it. `mmu` reads `SCTLR_EL1.M` back rather
than asserting it, and is printed through a device block of the table
just installed: if the mapping were wrong, the line would not appear.

## `Published`, not a lock

Values the handlers need are published once at boot, on the boot core
with interrupts still masked, and only read afterwards from interrupt
context. There is nothing to contend with, so a lock would be a second
thing to get wrong for no protection. The `Sync` impl rests on
write-once-before-unmask.

## The handler's two invariants

The claim/complete pair brackets everything. Until `complete`, the
controller delivers nothing more at this priority, so a path that returns
early without it stops the system dead.

Rearming the timer is load-bearing in the other direction: it asserts its
output for as long as its countdown is negative, so a handler that
acknowledges without rearming is re-entered the instant it returns,
forever, a livelock that reads as a hang.

Console input is queued and left. Echoing and dispatching happen in the
idle loop: a command run inside the handler would hold the interrupt
active, silencing the console for as long as it took and stopping the
timer with it.

## Faults stop

`banner::fault` stops rather than returns: nothing that reaches the
vectors today is recoverable, and resuming into the instruction that
faulted would fault again, forever, with the console filling up. A fault
before the console exists has nowhere to report and simply stops.
`halt` uses `spin_loop`, which emits the architecture's yield hint, so a
parked core stops burning power.
