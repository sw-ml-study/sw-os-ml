# mlsh

The MLOS inspector shell: the verbs a person or a boot script uses to poke
the object manager and read back what it did. Kernel-side, `no_std`.
Linked from `crates/mlsh/src/lib.rs`, `commands.rs`, `acquire.rs`,
`objects.rs` and `report.rs`.

## What kind of shell

It closes `docs/PRD.md` gate G1, which asks the proof of concept to reach
a shell. It is not a Unix shell, and never will be: MLOS has no filesystem
to navigate and no processes to list. What it has is a memory map it had
to work for, some devices it discovered, and, since M2, an object table.
Those are what a shell here is for. The verbs are deliberately few and
deliberately about the state MLOS actually has; `mlsh` is an inspector,
and it grows a verb when the kernel grows something worth inspecting.

## Input through the queue, commands from the idle loop

Input arrives through `mlos_queue::push` from the interrupt handler and is
drained by `Shell::pump` from the idle loop, rather than being dispatched
inside the handler: running a command inside a handler holds the
interrupt active and stops the timer. Everything `pump` does therefore
happens with interrupts enabled and the timer still running.

`Shell::run` idles with `wfi` rather than a spin: everything that happens
from there begins with an interrupt, so there is nothing to poll for and
no reason to burn a core doing it.

Echo happens in `Shell::feed` rather than in the driver because echo is a
property of the line being edited: a backspace has to erase a character
the terminal already drew, and only something that knows whether the line
is empty can decide whether to.

## Boot scripts: `mlsh.run=` and `mlos.rev=`

The shell reads two settings out of `/chosen/bootargs`. `mlsh.run=a;b;c`
runs those verbs at boot, before the prompt, which is what lets a headless
capture drive the shell at all: a log file is not a terminal, so no
keystroke ever reaches the guest. The verbs are echoed as if typed, so a
captured console reads the same as a session somebody sat through.
`mlos.rev=<sha>` is the commit the host built from, stamped into a layout
document's provenance, because the kernel has no other way to know what
produced it.

`mlsh.run=` takes the rest of the string, so it must come last. Its
commands take arguments, and arguments have spaces in them.

## Acting verbs and looking verbs are separate modules

`objects.rs` holds the verbs that make the object manager do something;
`report.rs` holds the ones that only look. The distinction matters more
here than it usually would: `sweep` and `get` change residency, so running
one changes what the other reports, and knowing which is which is the
difference between exploring a system and disturbing it. Nothing in
`report` changes residency, which is what makes those verbs safe to run
while working out what the acting verbs did.

## `dispatch`

The "needs a model" guard is a match arm rather than an early return
because it is a dispatch decision like the others, and it belongs where
they are. The verbs that need one are a constant, `NEEDS_MODEL`, so the
check is one line and the apology lives in one place; three copies of it
would be three places to change.

## `get` and `evict`

`get` is the verb that makes the fault path pokeable. Running it twice on
the same tile is the shortest possible demonstration of what an object
table is for: the second time costs nothing. `evict` makes the other half
pokeable: `get 3 7`, then `evict 3 7`, then `get 3 7` again, and the third
costs what the first did, which is the shortest demonstration that the
bytes really went away.

The residency is read before the acquire, because afterwards every object
is resident and the interesting fact, whether this one had to be fetched,
is gone.

The first byte is printed because it is the cheapest possible proof of
provenance: the stub tier fills with `layer ^ tensor`, and the disk image
sets a high nibble (`0xA0`) the stub never writes, so a byte with it set
can only have come off the disk.

## `model [KiB]`

`model` registers with the default budget, `model 8` with eight kibibytes.
The budget is the knob worth having, because the whole subject is what
happens when memory is smaller than the model.

## `sweep` is timed

A sweep is the only workload MLOS has and the elapsed figure is how every
later claim about overhead gets checked. The clock is `Facts::clock`, the
ARM generic timer's counter at 62.5 MHz on QEMU `virt`, not the 2 Hz timer
tick: a whole sweep happens between two ticks, so the tick cannot resolve
anything the object manager does, while the counter makes "how much does
recording an event cost" a question the shell can answer rather than
assert.

The figure is fixed point to the nanosecond, not rounded microseconds. A
sweep that faults is milliseconds and a sweep that only hits is a few
microseconds, and a unit that reads the first one well throws the second
one away, which is exactly the sweep that can measure what anything on the
fault path costs.

Stopping early is not a failure. The arena is smaller than the model, and
running out is the honest outcome when nothing evicts: it is the problem
M3 exists to solve, and how far the sweep got is the number that gets
compared.

## `replay` must match the simulator exactly

The counts `replay` prints and `mlos-sim`'s for the same trace and budget
must match exactly. They are integers decided by a sequence of decisions,
so a disagreement of one means one of the two is wrong, which is why there
is no tolerance and no rounding.

## `stream`

`stream` declares the model's access order; `stream N` advances it by N.
The verb exists so the one field no page-based system can hold, when an
object is next wanted, can be watched being written, in `objs` and in the
layout document, by something other than a test.

## `objs`

The inspector `docs/design.md` s.9 asked for. A page table would have
nothing worth listing: present, dirty, accessed. This has a tier, a
residency, and how often each object has been wanted, which is the state
every policy in `docs/PRD.md` reads.

Resident tiles only by default: a full listing of 128 identical cold tiles
says nothing, and the ones in memory are the ones a decision was made
about. `objs all` lists everything. The last column is `next_use`, and it
reads `never` until something declares a stream, because nothing else ever
writes it.

## `arena`

Reports the largest single run, not the total free. After evictions those
differ, and the difference is memory the arena holds and cannot give to
anything; a policy evicting perfectly into a fragmented arena has not
helped.

`Rm` is the same fact against the model rather than the buffer, and the
ratio `docs/PRD.md` s.5.2 says the whole system optimises. A small
fraction of a large model resident is the good case, not a failure.

## `faults`

The last fault is reported by layer and tensor, not by model number: only
one model exists, so its number is noise, and the layer and tensor are
what a page fault could never have told you.

## `mem` and `dev`

In `mem`, the two non-usable entries are the point: the kernel image and
the device tree blob are memory the machine has and MLOS may not hand out.
Everything the object manager will ever do starts from this number being
honest. `mem peek ADDR` reads eight bytes at ADDR; on an unmapped address
the CPU faults and the trap report is the answer. That is how a trap
report is provoked on purpose and read back.

Every line of `dev` comes from `Facts`, none from an assumption about the
architecture: the console kind is the one `boot` chose (a PL011 or virtio
on aarch64, a 16550 on x86-64), and a timer the platform has not described
is reported as absent rather than as irq 0.

## `Platform`

What differs by platform beyond addresses and numbers is one value: how
`dev` names the timer and interrupt controller, and whether `mem peek` can
read. A kernel that has nothing new to say sets one field to
`Platform::GENERIC`, which is the aarch64 kernel as it was before these
fields existed. `peek` is supplied by the kernel because it is `unsafe`
underneath and the shell is not where `unsafe` lives. `timer` is what the
timer is and what `timer_irq` counts in: `generic, irq` on aarch64,
`lapic, vector` on x86-64, where the LAPIC timer has a vector and no IRQ
line.

`Facts::ticks` is borrowed rather than copied because it keeps changing,
and the shell should report the count at the moment it was asked, not at
the moment boot handed the facts over.
