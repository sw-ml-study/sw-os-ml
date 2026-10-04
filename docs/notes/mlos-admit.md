# mlos-admit

Admission control: whether a budget can honour one more contract beside
the ones it already admitted, and why not when it cannot. Objects layer,
above `mlos-session`. Linked from `crates/mlos-admit/src/lib.rs`,
`latency.rs` and `refusal.rs`.

## Why its own crate

`mlos-session` was at four modules when M5 step 002 needed a fifth
concern, which is the point at which this repo creates a sibling crate
rather than a fifth module. The split is also the right one: a session
record knows nothing about budgets, and admission knows nothing about
slots beyond asking how many are live. The manager owns a `Capacity` and
asks it before `Sessions::create`; the simulator and the boot tests ask
the same `Capacity` on the host.

## What "can be honoured" means

`docs/design.md` s.5.3 says `ml_session_create` may refuse and that
refusing is a normal outcome; `docs/architecture.md` s.5 puts it on the
ladder as rung L6. This crate says what the refusal is measured against.

**In bytes.** A contract declares the `context` it will hold, in tokens.
That context becomes KV, `kv_per_token` bytes of it per token over every
layer, and that is what the session is charged for at admission: the
bytes it *will* hold, not the ceiling it may not pass. The budget
reserves the per-token working set of the weights first (`reserved`),
because every session needs that resident in its turn and nobody owns
it. A contract is honourable in bytes when its need fits what the budget
has not yet promised to the sessions already live, and when its own
`resident_ceiling`, if it set one, is not below its own need: a session
whose ceiling cannot hold its context could never be served and is
refused before anything else is asked.

**In latency.** The G5 report (s.4) measured what a latency ceiling costs
the other sessions: a ceiling of a quarter token halves one session's
period for five percent more reads overall, and below that the session
runs a whole token ahead, alone, and every weight it reads alone is one
the others then miss -- eight times the reads at 128 and 32. So a nonzero
ceiling below `token / 4` is refused. And ceilings compete: when `k`
contracted sessions all come due at once, each is served in turn and the
last waits for `k - 1` tokens, so every ceiling among them must cover
`(k - 1) * token`. A newcomer that would break a promise already made is
the one refused, naming the ceiling it would have broken.

`token` is the acquires one token takes in weights, which for the
synthetic model is every tile once (`mlos_lab::SYNTHETIC`); the KV
accesses grow with context and are left out of the unit on purpose, the
way `latency_ceiling` itself is measured in acquires.

## `Ss` becomes a capacity figure

M4 measured `Ss` as the live count over the arena's capacity, and said so
(`docs/notes/mlos-metrics.md`). `Capacity::admits` is the figure the PRD
meant: how many sessions like the live ones the budget admits, the room
after the reserve divided by the mean need of the sessions it is serving.
When nothing declared a context the mean is zero and the answer is the
live count, which is why every M3 and M4 replay line is unchanged to the
character: a replayed trace runs under `Contract::NONE`.

## Refusals say why

`Refusal` is `Slots`, `Ceiling { ceiling, need }`, `Bytes { need, free }`
or `Latency { ceiling, least }`, each naming the figure that failed and
the figure it was held to, with a `Display` the shell prints after
`refused:`. It converts to `Error::Refused` for anything that only has an
`Error` to return. Four variants rather than one because the shell user
and the G6 report both need to know *which* rule said no.

## Kernel equals host

`crates/mlos-cli/tests/admission.rs` boots both architectures with seven
`session new` lines chosen so each rule refuses once, and predicts every
printed line from the same `Capacity` on the host, then compares the
`admits` figure. Both agree. The arithmetic is integer and saturating,
with zero in `bytes` or `token` turning that test off, so `NONE` admits
anything a slot is free for.
