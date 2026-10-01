# mlos-policy

What to throw away when memory runs out: the `Policy` trait, the
known-next-use policy this project exists to test, and the three baselines
it is measured against. Policy layer; `no_std` and pure. Linked from
`crates/mlos-policy/src/lib.rs`, `residency.rs`, `nextuse.rs` and
`baselines.rs`.

## Pure, and stateless by rule

`docs/design.md` s.2 makes it a structural rule: a policy takes the object
table's view and returns a decision. It performs no I/O and holds no state
the table does not own. That is the whole reason the same code can run in
the kernel and in `mlos-sim` against a recorded trace, and it is why every
`Policy` method takes `&self`.

The rule has a price, paid deliberately. An LRU that cannot keep its own
list has to find the least-recently-used object by looking, which is a
scan of the resident set per eviction. A kernel can afford that at these
sizes and could not at a million objects. Known-next-use has to scan as
well, since it is looking for a maximum, so the cost is the same for the
policy being tested and for the ones it is tested against, which is what
keeps the comparison fair even though it is not what keeps it fast.

What a policy reads is `ObjectMeta`, and every field it needs is already
there: `used_tick` for recency, `placed_tick` for insertion order,
`next_use` for the future, `reload_cost` and `recompute_cost` for what
being wrong would cost.

## Declining is an answer

`Policy::victim` returning `None` means decline, and declining is a
legitimate answer, not a failure. Demand paging never evicts anything, and
the cost of that is exactly what the comparison is measuring. A policy may
also decline because every candidate is worse than refusing: an object
that cannot be recomputed and is wanted next is not a victim at any price.

`victim` is called repeatedly while there is still not enough room, so a
policy naming one victim at a time is normal and does not need to know
how much more is needed.

## `Residency`: the half of the interface the kernel must satisfy

`Residency` is its own module because it is the half of the interface the
kernel has to satisfy, over an open-addressed table, while the simulator
satisfies it over a `Vec`. Everything about the shape of the trait is a
constraint on the kernel rather than a convenience for the policy.

It is indexed rather than iterable so the kernel can satisfy it over an
open-addressed table without materialising anything. A policy that wants
the whole set walks `0..len()`, and pays for it. `at` must be stable, not
sorted: the order must not change between two calls with nothing in
between, or a policy would return different victims for the same state and
the replay would stop being deterministic.

`now` is on `Residency` rather than passed to `victim` because it is a
fact the table's owner holds, and `docs/design.md` s.2 says a policy
reads what the table owns. It is what turns `NextUse::At`, a position,
into the distance a policy actually compares, and doing that subtraction
here, for the few objects being weighed, is what lets advancing the
stream stay a single increment.

## Known-next-use

The policy this project exists to test. Belady's rule is normally
unimplementable because it needs the future; a dense transformer hands it
over, because the order it will read its own weights follows from its own
structure. Three things it must get right that a naive reading would not.

**Cost is half the decision.** Belady's rule assumes every miss costs the
same, which is true for pages and false for everything MLOS holds. A
weight tile cannot be recomputed at any price; an activation is cheaper to
rebuild than to keep. The furthest-away object is the wrong victim if it
is also the dearest to get back, so what is maximised is distance per unit
of recovery cost rather than distance. The recovery cost is the cheaper of
fetching again and rebuilding. `CostNs::IMPOSSIBLE` is what weights carry
for recompute, so taking the minimum is what makes an activation, cheap to
rebuild and expensive to store, a better victim than a tile at the same
distance. A zero cost is treated as one so the division is defined.

**A guess is not knowledge.** `NextUse::At` is exact: a declared stream
said so. `NextUse::Probability` is a router's distribution, and acting on
it as though it were a distance would licence evicting something the
system was merely unsure about as though it knew. Both are turned into a
horizon (an expert wanted with probability `p` each step is expected about
`1/p` steps away), and the probabilistic one is then halved (`HINT`), so a
hint has to be twice as good as knowledge before it wins. That is the
discount made explicit rather than left in a comment.

**`Never` is not infinity.** An object nothing has declared a future for
is the obvious victim, and it is obvious because nothing knows about it,
not because nothing will want it. It gets the `UNDECLARED` horizon
(`1 << 40`): large enough to outrank any real distance in this workload
and far enough from `u32::MAX` that multiplying by `SCALE` cannot
overflow. That is a decision with a reason rather than a maximum.

A position already passed saturates to a distance of zero: the object is
wanted now, and is the worst possible victim.

**Shared objects are dearer (M4 step 002).** The recovery cost is
multiplied by `share_count`, the number of leases holding the object,
with zero counting as one: evicting an object k sessions hold means k
reloads, one per holder, when each comes back for it. This is the first
place the parameter-major idea touches the policy -- what is shared is
worth keeping -- and it is a factor on cost, not a veto, so a shared
object far enough away is still evicted before a private one wanted now.
Replayed traces and sweeps hold Streaming leases for one access and
release them, so their counts are unchanged by this.

## `SCALE`

Recovery costs are nanoseconds and run to millions, so a small fixed-point
scale truncates the whole ratio to zero for anything expensive. The first
version used a thousand; every weight tile nearer than four thousand steps
scored exactly zero, and the policy was choosing between ties by table
position. `SCALE` is now a million: large enough that the distance still
resolves after the division, small enough that the multiply cannot
overflow.

## The baselines

Demand, FIFO and LRU are together because they are one thing: the numbers
known-next-use has to beat. Demand paging is the floor. It never evicts,
it refuses, and what refusing costs is part of what the comparison
measures. FIFO and LRU are the two a page-based kernel actually ships.

FIFO and LRU are one type, `Oldest`, reading different clocks, because
that is what they are. Both evict the resident object with the smallest
tick; they differ only in whether that tick is when the object arrived
(`placed_tick`) or when it was last wanted (`used_tick`). Written as one,
the comparison between them is exactly a comparison of which clock
matters, and they come out identical whenever nothing is reused, because
then the two clocks are the same clock.

FIFO knows nothing about use. An object fetched once at the start and
wanted on every access since is still the first thing it throws away,
which is the failure LRU exists to fix. LRU is a good guess whenever the
recent past predicts the near future. A cyclic sweep is where that stops
being true: the object LRU just used is the one that will not be wanted
again until the cycle comes round, so it evicts precisely what it is about
to need.

None of them keeps a queue or a list. Each finds its victim by scanning
for a minimum, which is what the kernel would have to do over its own
table, and what known-next-use does when it scans for a maximum.

## Lessons

**Ties go to the lower id, in every policy.** The answer must not depend
on the order the residents were offered in, and it did: the kernel offers
hash-slot order and the simulator insertion order, and the two evicted
different `Never` blocks of the same session. That was harmless in a
byte-counting simulator, and five reads apart once fragmentation was real
on both sides. Known-next-use breaks score ties on the lower `ObjectId`.
`Oldest` does the same even though ticks are unique (one acquire per
tick) and the tie-break never fires there; it is written anyway so that
every policy in this crate answers independently of enumeration order,
and a future policy must too.

**Check the fixed-point scale against real costs.** A ratio that
truncates to zero for the expensive objects turns a cost-aware policy into
a table-position policy without any test failing. See `SCALE` above.
