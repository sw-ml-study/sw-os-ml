# mlos-events

Residency transitions recorded as they happen, in a fixed ring the shell
prints as JSON Lines. Objects layer. Linked from
`crates/mlos-events/src/lib.rs`, `event.rs` and `line.rs`.

## Why a stream and not just a snapshot

A snapshot says what is resident now. The event stream says what happened
to get there: what came in, what it cost, and what had to be refused. It
is the part of the shared visualization demo that is MLOS's alone.
SWTOS's flash is static once built, whereas the whole subject of an ML
object store is churn.

## Recording does not format

The fault path pushes a `Copy` struct into a ring and returns; turning it
into JSON happens later, in the shell, off the path being measured.
`Ring::record` is a handful of stores and a modulo: nothing is formatted,
nothing is allocated, nothing is written to a device. That is what makes
it cheap enough to leave on while the thing being measured is the fault
path itself. An emitter that wrote to a console from inside `service`
would be timing its own console driver and calling the result a fault
cost.

`Ring::enabled` is a public field so `trace off` is a field write. The
switch exists so the cost of recording can be measured, sweep with it
off, sweep with it on, compare, rather than asserted to be small.

## The ring is owned, not global

The ring belongs to the `Manager` rather than being a global, so a test
can have one of its own. That matters more here than it usually would:
the thing under test is a side effect, and a global would make every case
in a test binary share it.

## Capacity and overflow

`CAPACITY` is 256. A sweep of the synthetic model is 128 acquires and so
at most 128 events, which leaves room to spare. Overflow drops the oldest
and is counted, never silent: a consumer replaying a stream with a hole in
it would rebuild the wrong picture and have no way to know. The count is
emitted as a `dropped` line shaped like any other event, so a consumer
reading line by line does not need a second shape for it, and it is never
omitted when zero. "No dropped line" and "a dropped line saying zero" are
the same fact only if you already trust the emitter.

## No wall clock

There is no timestamp in an event, deliberately. The only clock the shell
has is the 2 Hz timer, which cannot resolve a fault; and elapsed time
under TCG is not the timing of any real machine. So ordering comes from
`seq`, which is exact, and duration comes from `cost`, which is the
modelled figure a provider charges: the same axis M3's policy comparison
is measured on.

## Constructors, recorded once

`Event` has a constructor per kind rather than a struct literal at each
call site, because the fault path is not where the shape of an event
should be decided. It is also what lets the fault path record once, after
the outcome is known, instead of recording a hope and amending it; an
amendment is a second thing to remember on a path that already has two
exits.

Per kind:

- `Refused` is not a failure. Until M3 there was no policy to choose a
  victim, so refusing was the honest outcome, and it is the most
  interesting event in the stream: the moment the system ran out of the
  resource it exists to manage. `bytes` is zero because nothing moved,
  but the cost is kept, because what a refusal would have cost is the
  number an admission policy is deciding against.
- `Evicted` was reserved from the start and emitted by nothing until M3
  step 008. Its `bytes` is what came back, which is the number that makes
  a residency curve add up.
- `Hit` is the cheap case, the one a residency policy exists to produce
  more of. It carries no cost because nothing was fetched, which is the
  point of recording it at all.

`session` is carried so an access trace derived from a stream can say who
made each acquire rather than assume. There is one session today; the
assumption would be right and would stop being right at M4, silently, in
a file somebody was measuring from.

## The line format

JSON Lines behind a `@ev ` marker. The marker is what lets a consumer pull
the stream out of a console it shares with the shell's prose without
parsing the prose: `grep '^@ev '` is the whole extractor. It also keeps
the stream distinct from a layout document, which is the only other JSON
on that console and begins a line with a brace.

Self-delimited per line, deliberately: a capture that was cut short
mid-run still yields every complete line before the cut. A single JSON
array would yield nothing at all without its closing bracket, and a
truncated capture is the normal case, since `mlos run --capture` kills
the guest after a fixed number of seconds.

Every key is present on every line, including the ones that do not apply.
A consumer reading columns does not have to handle a missing field, and
the cost is a few bytes on a stream that is already bounded by how much a
console can carry.

`why` maps error codes to words with no wildcard arm, so a new error code
is a compile error here rather than an event that says nothing about what
went wrong.
