`mlos-policy`: demand, FIFO and LRU. The numbers next-use has to beat.

Three baselines, smallest first. Demand paging with no eviction at all is what
MLOS does today and it is the floor: it does not evict, it refuses, and the
comparison should show what refusing costs. FIFO and LRU are the two an
ordinary operating system would bring.

**These are also the harness's own test.** On a workload with real reuse, LRU
must beat FIFO; if it does not, `mlos-sim` is wrong and every number after this
step is worthless. Say so explicitly in the summary with the figures, and if
they come out the wrong way round, fix the harness rather than the expectation.

Run them against the step-001 trace and record the table even though that trace
is the easy case -- a dense sweep with no reuse is exactly where LRU is pessimal,
and seeing it be pessimal for the right reason is worth having on the record
before step 004 makes the workload fair.

Each policy is a crate the kernel could link: `no_std`, no allocation, and
deciding from what the object table already holds. A policy that needs a data
structure the kernel cannot afford is not a baseline, it is a different system.
