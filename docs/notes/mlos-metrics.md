# mlos-metrics

Running per-class counters and the report read out of them: what MLOS
counts instead of throughput. Objects layer. Linked from
`crates/mlos-metrics/src/lib.rs` and `report.rs`.

## Why these numbers and not throughput

A conventional kernel reports throughput and residency: how fast, and how
much memory. Those are the right numbers when RAM is cheap and the
workload is unpredictable. Neither holds here, so MLOS counts different
things (`docs/PRD.md` s.5.2) and the objective is useful work per
resident byte, not work per second.

## Per class, always

"40,000 faults" is a number; "38,000 of them cold KV blocks" is a
diagnosis, and it is the difference between knowing the system is
thrashing and knowing what to do about it. Every counter that can be
split by `ObjectClass` is, and `Report::worst` names the class that
faulted most because that is the single most useful line of a report:
it says what the system is actually struggling with, which a total never
answers.

## Faults are counted twice: how often, and how much

`Counters::fault` records both the count and the bytes, because they
answer different questions. The count says how often the system was wrong
about what it would need; the bytes say what being wrong cost. A thousand
faults on scales is cheap where ten on experts is not.

## Saturating, never wrapping

Every counter saturates. A counter that wraps turns a long run into a
smaller number than a short one, which is worse than a counter that
admits it stopped: these are read to compare runs, and a wrap would make a
comparison quietly wrong rather than visibly stuck.

## Resident is signed

`Counters::resident` takes a signed delta because it has to work in both
directions the moment eviction exists; a counter that only went up would
make the first eviction look like a leak.

## Residency in parts per thousand

`Report::residency_per_mille` is per mille rather than a float, because
it is read in a kernel with no floating point and printed by a shell with
no formatter. The ratio matters more than the precision: 3/1000 resident
is the interesting fact, not whether it is 0.31% or 0.34%.

It is `None` when nothing is registered. A ratio with no denominator is
not zero, it is unanswerable, and reporting zero would read as "nothing
is resident" rather than "nothing exists". `registered` is the
denominator of `Rm`: what the model is, against what of it is in memory.

## The headline numbers (M4 step 005)

`docs/PRD.md` s.5.2 names two headline numbers and this crate computes
them in one place, `Headline::of`, from raw counts, so the kernel (from
`Counters`) and the simulator (from its `Outcome`) cannot mean different
things by them:

- `Ps`, parameter applications per parameter read: weight bytes served
  (hits and reads) per thousand weight bytes read from a provider. One
  thousand is one use per read; four thousand is four sessions sharing
  each read, which is what parameter-major scheduling is for. Bytes stand
  in for parameter values; the ratio is the same.
- `Ss`, sessions per GiB of resident budget, in thousandths. Before
  admission control exists (M5) it is measured at a fixed budget: the
  sessions live over the arena's capacity, not the most the budget could
  admit. Said here so nobody reads it as a capacity figure yet.
- `Ks`, KV bytes resident per live session, which comes free from the
  per-class `held` counter.

To compute them the counters grew `hits` (bytes served without a read,
per class) and `held` replaced the single `resident` total (bytes
resident per class, signed). `Report::resident` is now the sum of
`held`, and `Rm` is unchanged. The replays print all three on their count
line, the simulator's from `Outcome::headline` and the kernel's from
`Report::headline` at the end of the run before the sessions are
destroyed, and the boot test compares the lines as strings.

**A fault is counted when its bytes arrive.** Until step 005 the manager
counted a fault, and its bytes as fetched, the moment a miss was
described, before any provider was asked. Demand paging refuses most
misses, and every refused miss was counted as kilobytes fetched: the
kernel reported `Ps` 1,302 where the simulator, counting bytes actually
read, reported 24,000. The count now happens after a successful
placement. A refused miss is a refusal -- the event stream and the replay
line say so -- and not a fetch, which is what `Pf` and `Ps` need it to
be. `faults` in the shell therefore reports misses that were served.
