# MLOS Objects

Vision: the kernel stops managing memory the way every operating system
does and starts managing ML state the way none of them do.

M1 produced a small aarch64 kernel: it boots, maps memory, takes
interrupts, and answers a shell. Nothing in it is about machine learning.
This saga adds the thing the whole project is for -- an object table whose
entries are weights, KV blocks, experts and activations, with residency,
providers, leases and a fault that carries meaning.

Gates G2 and G3 from docs/PRD.md:

- G2: a synthetic transformer's weights registered as ML_OBJECTs across
  at least three tiers, resolved through providers.
- G3: touching a non-resident object raises MODEL_FAULT carrying
  {class, model, layer, tile}, the provider services it, and the fault is
  counted per class.

The ObjectId bit layout was fixed in M1 step 002 precisely so this saga
would not have to move it.

DEFERRED, deliberately: userspace and the syscall surface. docs/plan.md
put them before the model fault, and that was the wrong order. Neither
gate needs EL0: a fault is a kernel mechanism, and the object manager is
the thing under test. Building a process model first would be weeks of
POSIX-shaped machinery in front of the experiment. Userspace arrives when
sessions need isolating from each other, which is M4.

1. **objtab** -- the object table. Open-addressed on ObjectId, sized from
   the residency budget, in a kernel arena. Host-tested: the table is
   pure data structure and needs no VM to exercise.
2. **provider-trait** -- Provider (resolve/read/prefetch/cost) and the
   DRAM provider. `cost()` is not optional: an eviction policy that
   cannot ask what recovery costs is guessing, which is what LRU does.
3. **model-fault** -- acquire, miss, MODEL_FAULT{class,model,layer,tile},
   provider services it, lease installed, resume. The fast path must not
   cross into a service.
4. **metrics** -- per-class counters. `Pf` broken down by class, `Rm`,
   `Bt`. "40,000 faults" is meaningless; "38,000 of them cold KV" is a
   diagnosis.
5. **synthetic-model** -- 8 layers x 16 tiles across DRAM, a block store
   and recompute; swept from mlsh. Gates G2 and G3, visibly.
6. **blockstore** -- a virtio-blk provider, so objects can come from
   somewhere that is not RAM and the tiers are real rather than
   simulated. M1's virtio-mmio transport already exists.

Parked until this saga lands:
- Eviction policy. There is nothing to evict until residency is tracked.
- Streams and next-use, which are M3 and the actual thesis.
- Userspace, per above.
