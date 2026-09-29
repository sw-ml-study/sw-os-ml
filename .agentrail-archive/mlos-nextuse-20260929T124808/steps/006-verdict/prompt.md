Stop, run the comparison, and look at the table.

Not the G4 report -- that comes later, with a real model and in-kernel numbers.
This is the decision point: four policies, one workload with genuine reuse in
it, one budget, and an answer to the only question that decides whether the rest
of this saga is worth doing.

Run demand, FIFO, LRU and known-next-use against the step-004 trace. Report
provider reads and bytes moved for each, **at several budgets** -- a policy that
wins only at the budget it was tuned for has shown nothing, and the shape of the
curve says more than any single row. The budget where LRU catches up is a real
finding and should be reported as one.

Then say which of these happened, in the summary and in `docs/status.md`:

1. **Known-next-use separates clearly.** The thesis holds so far. Continue to
   the syscalls, the evictable arena and the in-kernel port, and note what the
   separation was so the later numbers can be compared against it.
2. **It separates, but only marginally, or only at some budgets.** Say where and
   by how much. A small separation is a real answer and it changes what M4 and
   M5 are worth -- an operating system is a large thing to build for a few
   per cent.
3. **It does not separate.** Say so plainly and STOP. Do not re-cut the trace,
   do not adjust the weighting until it works, and do not proceed to
   `stream-syscalls`. Write down what was measured and what it means: the gift
   is not worth what it costs to accept, and the architecture should be
   reconsidered before four more milestones are built on it.

Whichever it is, `docs/PRD.md` s.9 Q1 and Q2 get answered by name, with numbers.

Before reporting, check the harness rather than the result. LRU must beat FIFO
on this workload; if it does not, the harness is wrong and the comparison is
meaningless whichever way it came out. And known-next-use must be at least as
good as every baseline at every budget -- it has strictly more information, so a
budget where it loses is a bug in the policy, not a finding about the thesis.

Put the table where someone can find it without reading a commit message.
