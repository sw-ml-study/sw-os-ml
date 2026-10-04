# mlos-session

Sessions as kernel objects: a record, a contract, a budget, and what each
one owns. Objects layer. Linked from `crates/mlos-session/src/lib.rs`,
`table.rs`, `budget.rs`, and `crates/mlos-objman/src/session.rs`.

## Why its own crate

`docs/design.md` s.5.3 gives a session four syscalls: create (which may
refuse), budget, contract, destroy. M3 left a session as a `u16` tag on
an access and an `owner` field on metadata, with nothing created or
destroyed. The record has to be shared by the kernel and the simulator,
like the policy, or the two stop agreeing the moment a session's
lifetime changes what is resident. `mlos-objman` was at four modules and
holds the manager-side half (`session.rs`: destroying a session evicts
what it owned); the record and its table are `no_std` and know nothing
about arenas.

## Sessions without processes

There is no userspace. A session here is a kernel-side record driven by
the shell and the replay, as the declared stream was in M3. That is said
plainly rather than hidden, and it is why a trace recorded from a real
inference engine is not in M4 either: it needs a process to record from.

## The contract, and which field is enforced

`Contract { quality_floor, latency_ceiling, resident_ceiling }` is
the shape `docs/design.md` names, with zero meaning "none". Only
`resident_ceiling` is enforced in this step: `Sessions::charge` refuses
an acquire that would take the owner past it, before any victim is
chosen, so a refused acquire evicts nothing. `latency_ceiling` is
read by the scheduler from step 004 (the latency escape); the quality
floor waits for M5's degradation ladder.

## Ids: created or adopted

`create` hands out the lowest free id from one upward, the way a syscall
would. `adopt` takes an id the caller chose, because a replayed trace's
sessions already have numbers and the simulator and the kernel must
agree on them. Zero is nobody: shared weights are owned by session zero
and counted against no one. An unknown owner is served and uncounted
rather than refused, so every M3 workload runs unchanged.

## Destroying a session takes its objects

`Manager::destroy_session` walks the table, evicts every resident object
the session owns, forgets the rest, and forgets the session. If an
eviction is refused (the arena's free list is full) it stops there and
the session stays, on the arena's own principle: nothing is lost by
accounting. The replay adopts every session the trace names before it
starts and destroys them all when it ends, which is how a finished
session's KV cache comes back.

## Sixteen

`MAX_SESSIONS` is sixteen: more than any workload here declares, and a
seventeenth is refused. That refusal is admission control's first, crude
form; M5 replaces the count with a contract.

## Promised against delivered (M5 step 001)

`Contract::quality_floor` is a `Precision`: the coarsest a session's
objects may be degraded to, with `Ternary`, the coarsest there is,
meaning anything goes; `Contract::permits` is the test the ladder will
ask. `Delivered` is what the session actually got, and every field only
moves toward worse, so `ml_session_contract` can answer for the whole
life of the session: the coarsest precision any of its objects was
served at (recorded by `charge`, which now takes the precision), the
most it held at once (`charge` again), and the longest a token took in
acquires from the end of the previous token to its own (`took`, fed by
both schedule drivers -- `merge_timed` on the host and `Lanes::order` in
the kernel -- which compute it the same way). `session` in the shell
prints both columns; `session new [KIB] [WAIT] [FLOOR]` sets all three
promises.

## Context, and what admission charges for (M5 step 002)

`Contract::context` is the tokens of KV a session says it will hold.
It is the one field admission charges in bytes: a ceiling is a promise
not to exceed, a context is what will actually be resident, and only the
second can be summed across sessions against a budget. Zero means
undeclared and uncharged, which is what every replayed trace runs under.
The admission arithmetic lives in `mlos-admit`, a sibling crate, because
this one was at four modules; `Manager::create_session` asks it before
`Sessions::create`.
