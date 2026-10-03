# mlos-sched

Whose turn it is: a scheduler sees every session's declared stream and
names the session to serve next. Policy layer. Linked from
`crates/mlos-sched/src/lib.rs`, `process.rs`, `parameter.rs`,
`merge.rs`, `lanes.rs`.

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

The cost of lockstep is latency: a session that has finished its token
waits for the slowest before it may start the next, and within a token
it moves at the group's pace.

## The latency escape (M4 step 004)

`At::waited` is the acquires served to other sessions since this one's
current token became due (its previous token ended, or the start), less
its own accesses in the token. Not the gap between two of its accesses:
under lockstep that gap is a handful of acquires while the token takes
four times longer, so a gap would never fire. `At::ceiling` is the
session's `Contract::latency_ceiling`, in the same unit, which is the
kernel's acquire clock -- the unit `next_use` is already in.

`ParameterMajor::pick` serves the lowest session whose wait has reached
its ceiling before applying the lockstep rule. Once overdue it stays
overdue (its wait does not shrink as it is served), so it runs the rest of
its token alone and starts the next the moment its wait allows: it has
left the batch and reads privately. A ceiling therefore bounds a token's
period at the token's own length plus twice the ceiling -- the wait before
it and the wait within it.

Measured on the real shape, four sessions, 1536 MiB, next-use
(`cargo test -p mlos-workload --test latency -- --ignored --nocapture`):
lockstep costs session 1 about 11% in mean period (1,208 against 1,086
acquires process-major). A ceiling of 512 brings it to 817 for 15% more
reads overall; 256 to 561 for 5% more; 128 to 433 for eight times more,
because the session now runs a full token ahead and every weight it
reads alone is a weight the others miss. The break-even for this
workload sits at a ceiling near a quarter of a token: below it the
escape costs the whole group what parameter-major had won.

## `merge`, host side only

`merge` drives a schedule over per-session token streams into one trace
for the simulator to replay, behind the `alloc` feature because it needs
a growable buffer. It is the only place a `Vec` appears; the kernel
links the crate without it. `mlos-workload`'s `trace()` is now
`merge(ProcessMajor, tokens)`, one generator instead of two.

## `merge_timed` and the `lanes` module (M5 step 001)

A contract promises a latency ceiling, so the record of what was
delivered has to hold the worst period a session actually saw. Both
drivers measure it the same way: a token's period is the number of
accesses served, to anyone, between the end of the previous token and
the end of this one. `merge_timed` returns that per lane beside the
trace; `merge` is `merge_timed` with the periods dropped, so the
workload generator did not change.

The driver's bookkeeping -- cursors, clock, when each token became due
-- moved from `merge.rs` into `lanes.rs` as `Lanes`, with `advance` the
only thing that moves a cursor and the only place a period is read.
That is a fifth module and a five-function module, both over the
`sw-checklist` gate; folding `Lanes` back into `merge.rs` gives a
five-function module there instead, which is the oscillation the gate
notes say to stop at. The exception is carried in the step 001 commit.

## What the KV cache puts on the gain

A session's cache is its own. Parameter-major shares weights and cannot
share KV, so its best case saves `(N - 1) / N` of the weight reads and
none of the KV reads. The G5 tables print the KV share of accesses next
to the gain for exactly that reason.

## In the kernel (M4 step 006)

`mlos-lab::lanes` implements `Waiting` over static buffers and runs
`ProcessMajor` or `ParameterMajor` over the disk trace's sessions before
the replay declares and replays the order. Nothing in this crate changed
to make that possible, which was the point of `Waiting` taking positions
and next objects and nothing else: the host's `merge` and the kernel's
`Lanes::order` are two drivers of one `pick`. The boot tests hold the
kernel's eight count lines to the simulator's, on both architectures.
