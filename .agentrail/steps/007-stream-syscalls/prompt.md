`ml_stream_declare` and `ml_stream_advance`: the calls by which userspace hands
the kernel its own future.

The syscalls that make the thesis expressible. A stream is a declared sequence
of objects a session will acquire in order; advancing it tells the kernel where
in that sequence the workload now is. From those two facts every resident
object's distance-to-next-use follows, and `ObjectMeta::next_use` finally gets
written by something.

That field has existed since M2 step 001 and nothing has ever set it. Its own
documentation says it is "the field that does not exist in any page-based
operating system, and the one that makes this whole design worth building". This
step is where that stops being an aspiration.

Three-way, and do not collapse it. A dense sweep yields `Distance(n)` -- exact,
and a policy may act on it with certainty. An MoE router yields
`Probability(p)` -- a hint, and a policy may only weight by it. Collapsing the
two into one number would licence a policy to evict something it was merely
unsure about as though it knew. The type already distinguishes them; the
syscalls must not undo that.

No userspace exists yet and none is needed: the calls can be exercised from the
kernel side and from the simulator. Building a process model first would be
weeks of POSIX-shaped machinery in front of the experiment, which is the same
judgement M2 made about deferring EL0.

Keep the fast path fast. Advancing a stream must not walk the whole table --
that is a per-token cost and the table is the thing it would walk.
