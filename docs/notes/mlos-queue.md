# mlos-queue

A byte queue from an interrupt handler to the loop it interrupted.
Kernel-side, below the shell. Linked from `crates/mlos-queue/src/lib.rs`.

## Why its own crate

Nothing about it is shell-specific: any device whose interrupt produces
bytes faster than something wants to consume them needs exactly this, and
a shell is only the first such consumer.

## Why the handler does not run the command

Bytes arrive in an interrupt handler and are consumed by the idle loop. An
interrupt handler is the wrong place to run a command: it holds the
interrupt active, so the console cannot report anything that happens
while it works, and a slow command stops the timer. The handler's whole
job is to move the byte somewhere and get out.

## Two atomics, no lock

Single producer, single consumer, and both are on the same core; the
producer just happens to have interrupted the consumer. That is what makes
two atomics sufficient and a lock unnecessary. `HEAD` is advanced only by
the producer and `TAIL` only by the consumer, each at an index the other
never writes, and the Release store on each index is what publishes a
slot to the other side. The buffer is `Sync` on exactly that argument.

The capacity is a power of two so the wrap is a mask.

## Dropping on full is the right failure

`push` returns `false` and drops the byte when 64 are already waiting. A
keystroke lost then is a keystroke nobody was going to read in time
anyway, and the alternative, blocking in an interrupt handler, is a hang.
