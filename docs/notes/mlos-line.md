# mlos-line

One line of input, in a fixed buffer. Kernel-side, below the shell. Linked
from `crates/mlos-line/src/lib.rs`.

## Why its own crate

Nothing about it is shell-specific: anything reading a line from a console
needs a buffer, a backspace and a limit. `mlsh` is only the first such
reader.

## Why the buffer is fixed

`CAPACITY` is 96 bytes. Nothing takes arguments yet, so that is generous.
It is fixed because there is no allocator, and it will still be fixed
when there is one: a shell that can be made to allocate by holding down a
key is a shell with a denial of service in it.

## The edges are reported, not hidden

`push` returns `false` when the line is full so the caller knows not to
echo a byte that was not accepted. `backspace` returns `false` when there
is nothing to remove so backspace at the prompt does not erase the
prompt.

## `as_str` cannot fail today, and does not panic anyway

Only printable ASCII is ever pushed, so the UTF-8 check cannot fail.
Returning `""` rather than unwrapping keeps a future change to that rule
from turning into a panic with no console to report it on.
