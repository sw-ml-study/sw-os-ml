# mlos-sched

Whose turn it is: a scheduler sees every session's declared stream and
names the session to serve next. Policy layer. Linked from
`crates/mlos-sched/src/lib.rs`, `process.rs`, `parameter.rs`,
`merge.rs`.

## Why its own crate

`docs/architecture.md` s.6 is the inversion the project was named for:
a process-major scheduler drags the model past memory once per session;
a parameter-major one passes the model once and serves everyone waiting.
Deciding whose turn it is touches neither the table nor the policy, and
it has to run in the kernel and the simulator alike, so it is a `no_std`
crate of two traits and two rules, like `mlos-policy`.

## Two traits

`Waiting` is what a scheduler may see: how many sessions, where each is
(`position`: its token and index within it, or finished) and what each
wants next. `Schedule::pick` names a session. A scheduler holds no state
the sessions do not, except the round-robin cursor process-major needs,
so the kernel can host either over its session table without an
allocator.

## Process-major, the baseline

`ProcessMajor` serves one session from the start of a token to its end,
then rotates to the next live session. It exists to reproduce, from
per-session streams, exactly the order `mlos-workload` used to write by
hand -- round-robin over sessions, each sweeping its whole token -- and
`tests/baseline.rs` in `mlos-workload` holds it to the G4 report's
integers at four budgets. The `started` flag is the whole subtlety: at a
token boundary a session is either about to start or has just finished,
and only the second means "rotate".

## Parameter-major, the inversion

`ParameterMajor` picks the live session with the lowest `(token,
index)`, breaking ties toward the object the most sessions want next and
then the lowest session. The lowest token is a barrier: nobody starts a
token until everyone has finished the last, so the model passes memory
once per token. The lowest index keeps sessions in lockstep within a
token, so when they reach the same weight they reach it together and
every session after the first hits. The cache phases are private and of
different lengths, so sessions drift apart by a few accesses inside a
layer and realign at the next weight; a session a layer ahead would wait
there anyway. Shared count breaks the remaining ties, and the lowest
session makes the order deterministic.

The cost of lockstep is latency variance: a session that has finished
its token waits for the slowest. Step 004's latency escape is where a
contracted session buys its way out.

## `merge`, host side only

`merge` drives a schedule over per-session token streams into one trace
for the simulator to replay, behind the `alloc` feature because it needs
a growable buffer. It is the only place a `Vec` appears; the kernel
links the crate without it. `mlos-workload`'s `trace()` is now
`merge(ProcessMajor, tokens)`, one generator instead of two.

## What the KV cache puts on the gain

A session's cache is its own. Parameter-major shares weights and cannot
share KV, so its best case saves `(N - 1) / N` of the weight reads and
none of the KV reads. The G5 tables print the KV share of accesses next
to the gain for exactly that reason.
