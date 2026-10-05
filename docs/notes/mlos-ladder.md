# mlos-ladder

The degradation ladder of `docs/architecture.md` s.5: what each rung does
to a session's cache, and the walk down it when the budget shrinks.
Objects layer, above `mlos-objman` and `mlos-admit`. Linked from
`crates/mlos-ladder/src/lib.rs`, `fate.rs`, `walk.rs` and `squeeze.rs`.
`Rung` itself lives in `mlos-objtab` beside `Precision`, so a session
(`mlos-session`) can record the one it is on without depending on this
crate; a first cut put it in a fifth `mlos-session` module.

## The rungs, as blocks

A session's context is cut into KV blocks (`Shape::per_block` tokens
each, one object per layer per block, oldest first). The newest
`Shape::hot` blocks are the hot window; everything older is cold. One
function, `Shape::fate`, says what every block is on every rung:

| rung | cold blocks | hot window |
| --- | --- | --- |
| L0 | full precision | full |
| L1 | Q8 (half) | full |
| L2 | Q4 (quarter) | full |
| L3 | the oldest span -- the older half of the cold blocks, when that is two or more -- becomes one Q4 summary block; the rest stay Q4 | full |
| L4 | as L3, and of the cold blocks not in the span (the candidates) the older half are dropped | full |
| L5 | all dropped: the context is the hot window | full |
| L6 | no session is put here: admission refuses newcomers | |
| L7 | no session is put here: the lowest-priority session is terminated | |

`need(rung)` is the sum of what each block holds; `cost(from, to)` is the
sum of what each block that is requantized or summarised held before. On
L0 `need` is `context * kv_per_token` exactly, what admission charged,
including a partial last block. Because the plan, the table rewrite, the
host's prediction and the real-shape measurement all call `fate`, they
cannot disagree about what a rung means.

## Declared and accounted, not performed

MLOS has no arithmetic for summarising a span or ranking retrieved
candidates, and no KV contents worth quantizing. Every rung is therefore
an accounting change: the object's `size` and `precision` change in the
table, a resident block gives the arena its tail back, the owner's
account and the `KvBlock` counter are credited the difference, and the
bytes the real transform would have read are added to
`Delivered::recomputed` -- the session's share of `Rc`. In the
simulator the trace reads what the rung leaves: the summary instead of
the span, half the candidates, the hot window alone.

Which part of a context is "retrieved candidates" is something a RAG
workload knows and MLOS does not. L4 treats the cold blocks the summary
did not take as the candidates and keeps the newer half. A workload that
declares its candidate boundaries would replace that guess; the rung's
place on the ladder and its accounting would not change.

## Quality floors

A contract's `quality_floor` is a `Precision`. `Rung::quality` places
each rung on that scale: L1 is Q8, L2 is Q4, and every rung that forgets
context rather than precision is `Ternary` -- only a session that said
"anything goes" may lose context. `walk::movable` is the one test, and it
never moves a session onto L6 or L7, which are done to the population, not to a
session. So a Q8 floor stops at L1 and an Fp16 floor never moves.

## The walk

`walk::next` is pure over `Capacity` and `Sessions`, so the host predicts
the kernel by running it. If the sessions' needs on their current rungs
fit the room after the reserve, nothing happens. Otherwise every session
that may go one rung deeper does, together: the next rung is the
shallowest any movable session can reach, so each step is one rung for
everyone it moves, and a session stopped by its floor stays where it is.
When no session can move, L6 is in force -- every session is as degraded
as its contract allows, and admission, which still counts live sessions
at their full declared context, has no room for anyone -- and the walk
terminates the lowest-priority session holding a context (L7), then asks
again.

Only sessions that declared a context take part. One that declared none
holds nothing admission counted, so degrading it frees nothing the walk
can see, and terminating it frees nothing either.

Lowest priority is the highest id. There is no priority field; the
highest id is the latest promise among those still live while ids are
not reused, and terminating the latest promise first is the least
surprising order for everyone else. A priority field in `Contract` would
replace the key without changing the walk.

## In place, not evicted

A quantized resident block stays where it is and gives the arena its
tail (`Arena::release` of the aligned difference, which the free list
accepts because it tracks holes, not allocations). Evicting it instead
and letting the next acquire fault the smaller form back would charge a
provider read for a transform that happens in memory, and would make a
rung look like it costs reads it does not. A dropped block is evicted and
removed from the table.

## Arriving with a context

`arrive` is admission plus a prefill: a session that declares a context
arrives with that context cached, a block per layer per position
registered and placed under its id. Before step 003 a declared context
was only a number admission charged; without blocks to rewrite, a rung
could not be seen to free anything in the kernel. The prefill is best
effort: a short one is reported by the count, not as a refusal, because
admission already said the session can be honoured.

## Kernel equals host

`crates/mlos-cli/tests/ladder.rs` boots aarch64 and x86-64, admits a Q8
session and an anything-goes one, squeezes the budget three times
(reaching L1, then L4 with the Q8 session held at its floor, then L7),
and predicts every printed line -- rung, bytes read, bytes held -- by
running the same `arrive` and `squeeze` on a host manager built like the
lab's. Both agree.

## Measured on the real shape

`cargo test --release -p mlos-workload --test ladder -- --ignored
--nocapture`. MiniCPM5-1B at F16, four sessions with 2,048-token prompts
in 16-token blocks, the newest eight blocks (128 tokens) hot, forty
rounds, 1,792 MiB, next-use, parameter-major. Each row puts every
session's prompt on that rung before decoding.

| rung | prompt KV per session | accesses | reads | bytes read | refused | Rc per token | Ks at end |
| --- | --- | --- | --- | --- | --- | --- | --- |
| L0 | 48.0 MiB | 327,750 | 13,438 | 1,885 MiB | 100 | 0 B | 48.7 MiB |
| L1 | 25.5 MiB | 327,750 | 12,699 | 2,166 MiB | 0 | 1,887,436 B | 26.2 MiB |
| L2 | 14.2 MiB | 327,750 | 12,699 | 2,121 MiB | 0 | 2,831,155 B | 15.0 MiB |
| L3 | 8.7 MiB | 186,150 | 7,035 | 2,099 MiB | 0 | 3,067,084 B | 9.5 MiB |
| L4 | 5.9 MiB | 114,150 | 4,155 | 2,088 MiB | 0 | 3,067,084 B | 6.7 MiB |
| L5 | 3.0 MiB | 39,750 | 1,179 | 2,076 MiB | 0 | 3,067,084 B | 3.8 MiB |

The L0 row is G5's (13,438 reads at four sessions, 1,792 MiB), which is
the check that the degraded model changes nothing it should not.

**L0 is not the cheap row.** It reads the fewest bytes because it
refuses 100 accesses outright: in the band the budget cannot place them.
L1 is the first rung that serves every access, and the bytes read rise
281 MiB because those accesses are now served. A refusal is a failure
to serve, not a saving, so L1 is strictly better for the workload here.

**Quantizing buys bytes, not reads.** L1 and L2 read the same number of
objects; the KV is smaller, not fewer. L3 to L5 cut accesses, because the
trace no longer reads what was summarised, dropped or truncated, and
reads fall with them.

**`Rc` is paid once.** It is the bytes the transforms read, amortised
here over the run's 100 tokens; requantizing the cold prompt reads it
whole at L1, half again at L2, and the span once more at L3. L4 and L5
read nothing: dropping is free, which is why they are below the
summary on the ladder only in quality, not in cost.

Where the walk stops for the same four sessions as the room for KV
shrinks (pure arithmetic, `where_the_walk_stops_as_the_room_shrinks`):
192 MiB holds L0; 128 MiB needs L1 (102 MiB); 96 and 64 MiB need L2
(57 MiB); 48 MiB L3 (34.9); 32 and 24 MiB L4 (23.6); 16 MiB L5 (12.0);
below that, L7.
