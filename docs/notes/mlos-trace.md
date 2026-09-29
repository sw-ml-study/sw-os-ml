# mlos-trace

An access trace: what a workload asked for, in order, and nothing about
what the system did about it. Measurement layer, shared by the host tools
and the kernel. Linked from `crates/mlos-trace/src/lib.rs`, `derive.rs`,
`read.rs` and `write.rs`.

## The trace is the question, not the answer

`mlos-events` records what the system did -- placed, hit, refused -- and
those are properties of the policy under test. A trace is the question; an
event stream is one policy's answer to it. Keeping them apart is what lets
the same workload be replayed against four policies and scored on the same
question.

So a trace carries a session and an `ObjectId` per access and nothing
else. No tiers, no sizes, no costs, no residency. Every one of those is a
property of the system rather than of the workload, and baking one into
the trace would mean each policy was being asked something slightly
different.

## The model is not in the trace, and that is a trap

An `ObjectId` names an object but does not say how big it is, and a
simulator cannot tell when a budget is full without knowing. Sizes come
from the model the trace was taken against: `mlos-synth` today, a `.spm`
sidecar later. The header names that model so replaying a trace against
the wrong one is caught rather than silently producing numbers about
nothing.

## `no_std` and allocation-free

The kernel replays the same traces (M3 step 008), so the crate has no
allocator. Parsing fills a caller-provided slice, never a `Vec`; rendering
writes to a `fmt::Write`. The header carries the access count so a caller
can size that slice before it starts, which is what makes parsing possible
at all without an allocator. Reading the file off a disk is the host's
business and is three lines wherever it is wanted.

Two small consequences of the same constraint:

- `derive::field` takes a `key` that carries its own quotes and colon
  (`"\"bytes\":"`) because building that string in the function would need
  an allocator.
- `Header` borrows its strings from the text it was parsed out of, so a
  header costs nothing.

## Derived from a recording, never hand-written

The first trace must not be a literal. `docs/architecture.md` s.12 names
hand-written traces as a risk: a trace written by the person hoping for a
result can encode the result without anyone meaning to. A recording
cannot.

An event stream is exactly a recording of acquires. Every `placed`, every
`hit` and every `refused` is one acquire that really happened, on a real
kernel reading a real device, in the order it happened.

`refused` counts, and that is the interesting case. The workload asked; a
policy declined. What the workload asked for is a property of the
workload, and under a different policy that same acquire might have
succeeded -- which is the whole reason a trace must not record what the
system did.

`dropped` does not count: it is the ring's own bookkeeping. A stream that
dropped anything is a stream with a hole in it, and a trace built from one
would be missing accesses with no way to tell. `from_events` refuses it
rather than truncating: a trace with an invisible hole would produce a
plausible wrong number, which is worse than producing none, and it would
be a measurement of a workload that is quietly not the one that ran.

## Strict on read

A trace is an input to a measurement. A parser that quietly skipped a line
it did not understand would produce a shorter trace and a plausible wrong
number rather than an error, so `parse` is strict about everything it can
be strict about: magic and version, header order, the promised count
against the lines actually present.

## Text, and compact anyway

One access per line, as text, for the same reason the event stream is
text: a trace nobody can read is a trace nobody will question, and the
whole value of M3's numbers is that somebody else can dispute them. It is
also diffable, which matters when a trace is regenerated and the question
is what changed.

A line is a decimal session and a 16-digit hex `ObjectId`: about twenty
bytes, against forty for the same thing as JSON. At roughly two hundred
accesses per token of a real model, the difference is megabytes over a
long trace and it costs nothing to read.

## `Access::EMPTY` rather than `Default`

Object zero does not decode to a class, on purpose, so that an all-zero id
is rejected rather than read as object 0 of model 0. A constant named for
what it is says "buffer fill" where `Default` would imply "a reasonable
access".
