The same policies, in the kernel, producing the same numbers.

`mlos-sim` and `mlos-kernel` link the same `mlos-policy-*` crates and replay the
same trace under the same budget. The counts must match. A policy that behaves
differently in the two is a policy neither result describes, and the simulator
would have been measuring something that never ran.

Exactly, not approximately. Provider reads and bytes moved are integers decided
by a sequence of decisions, and if the two disagree by one then one of them is
wrong. Do not accept a tolerance.

The trace has to reach the guest. `/chosen/bootargs` carries a boot script
today (`mlsh.run=`) and the console carries documents out, but a generative
trace is far larger than a boot argument. Decide how it gets in -- a second
virtio-blk device is the obvious answer and the transport already exists -- and
say why.

Under TCG rather than HVF, for the reason `crates/mlos-cli/tests/boot.rs` gives:
TCG is deterministic, and a number that came out differently on someone else's
machine would be worse than no number.

Watch for the thing that will actually go wrong here: the simulator has an
allocator and the kernel does not, so a policy that quietly allocated will fail
to build for the bare target rather than produce a different answer. That is the
good failure. The bad one is a policy that reads a field the kernel populates
differently -- `next_use` in particular, which the simulator can compute from
the whole trace and the kernel can only know from a declared stream.
