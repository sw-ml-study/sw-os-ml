# Many instances: a plan

Status: PLAN. Nothing here is built. Companion to
[plan.md](plan.md) M7--M9 and to
[architecture.md s.7.5](architecture.md#75-tiers-are-a-cost-graph-not-a-ladder).

MLOS is a **distributed** guest control plane
([PRD.md s.1](PRD.md#1-the-one-sentence-thesis)), and everything built so
far runs on one node. This is how it stops doing that, starting on one
Mac with two emulated guests and ending on a rack of mismatched
machines.

## 1. The questions, because they decide the design

A cluster is only worth building if it answers something. Three
questions, in the order they become answerable:

1. **Does knowing what a peer holds beat not knowing?** Two nodes, the
   same model, the same budget. One fetches a tile; the other wants it
   moments later. Does asking the peer beat going to storage?
2. **Is remote memory cheaper than local storage, and does a policy
   exploit it?** This is where `TIER` stops being a ladder. Node B's RAM
   across 10GbE may be nearer than node A's own SSD, and no total
   ordering can say so -- only per-edge cost.
3. **Does global placement beat independent local decisions?** N nodes
   serving one model too large for any of them. Compare a cluster whose
   nodes each run their own policy against one that places objects
   knowing the whole.

Question 3 is the end state and the only one that justifies the word
"distributed". Questions 1 and 2 are the steps that make it measurable.

## 2. Simulate first

`AGENTS.md` makes this a rule and it applies with more force here: **any
policy must be runnable against a recorded trace in a host-side simulator
before it goes in the kernel.** A distributed placement policy debugged
across two VMs is a policy debugged through a transport, a scheduler and
two consoles at once.

So **phase 0 is `mlos-sim` with more than one node**, and it needs no
emulator at all:

- A node is an arena budget plus a set of providers with costs.
- The tiers become a **cost graph**: every (node, resource) pair is a
  place an object can be, and every edge has a cost. Local RAM is an edge
  of cost zero; local SSD, a peer's RAM and a peer's SSD are edges with
  different costs, and which is cheapest is a property of the pair rather
  than of a global ordering.
- A placement policy answers "cheapest way to satisfy `acquire(X, on
  node A)` before its deadline", which may be *fetch from a peer*, *read
  local storage*, *recompute*, or *refuse*.
- The same `Policy` trait, because `docs/design.md` s.2's rule still
  holds: a policy the kernel cannot host is a policy that will be
  rewritten after the numbers exist.

This phase answers all three questions cheaply and wrongly -- wrongly
because every cost in it is modelled. Phase 1 replaces the model with
measurement.

## 3. Transport: what is actually available

Checked on this machine rather than assumed, and the obvious answer is
not available.

| Option | State | Verdict |
| --- | --- | --- |
| `ivshmem` (shared memory between guests) | **not in this QEMU's aarch64 build** | ruled out |
| `vhost-vsock` | **not in this QEMU's aarch64 build** | ruled out |
| `-netdev socket` / `stream` between two QEMUs | available | needs a virtio-net driver MLOS does not have |
| A host relay between two virtio-serial ports | available | **needs no new guest driver at all** |

Shared memory would have been the cheapest thing to write and it is not
there, so the plan does not depend on it.

### Phase 1: a host relay, because the driver already exists

MLOS drives virtio-serial today -- it is one of the two consoles. Give
each guest a *second* virtio-serial port and run a host process that
pumps bytes between them:

```
   MLOS guest A                     MLOS guest B
        |                                |
   virtio-serial                    virtio-serial
        |                                |
   host socket  <--- mlos relay --->  host socket
                   (measures, delays,
                    and records)
```

What this buys, and it is most of the value:

- **No new guest driver.** The virtqueue code, the transport and the
  device handshake are written and tested.
- **The object-exchange protocol gets designed and debugged** -- request,
  response, refusal, and what an object's identity means on another node
  -- without a network stack in the way.
- **The relay is where cost is measured and injected.** It can add
  latency to model a 10GbE link or a slow one, and it records every byte
  that crossed. A real NIC would make the cost real but unmeasurable
  without more instrumentation; the relay makes it both.

What it does not buy: any claim about real network behaviour. It is a
point-to-point pipe with no loss, no reordering and no congestion.

### Phase 2: virtio-net, for real links and real machines

A `virtio-net` driver, two guests joined by `-netdev socket`, and raw
Ethernet frames with a private EtherType. **No IP and no TCP**: two nodes
on a point-to-point link exchanging object requests do not need
addressing or reassembly, and adding them would be building the parts of
Linux this project exists not to build.

The work is incremental rather than novel -- the same virtio-mmio
transport and split virtqueue the block driver uses, with two queues
instead of one and a twelve-byte header. M2 step 007 is the size
precedent.

This is also the phase that reaches a second machine, because
`-netdev socket` connects across a LAN as readily as across a host.

## 4. Homogeneous first, then heterogeneous

Two different experiments, and running them in this order is what makes
the second interpretable.

**Homogeneous.** N identical guests: same arena budget, same modelled
tier costs, same model, same workload shape. Everything is equal except
what each node happens to hold. This isolates **coordination**: any
difference between the cluster-aware policy and independent local
policies is caused by knowing what peers hold, because nothing else
differs.

**Heterogeneous.** Now vary one thing at a time:

| Vary | Tests |
| --- | --- |
| Arena budget per node | Does a big node usefully act as cache for small ones? |
| Modelled storage cost per node | Does a node with a slow disk learn to ask a peer? |
| Link cost between pairs | Does the policy prefer a near peer over a far one? |
| Session load per node | Does an idle node hold state for a busy one? |

The last row is the interesting one and it is the cluster form of M4's
parameter-major inversion: one node sweeps the weights and serves several
nodes' sessions, rather than each node sweeping them itself.

## 5. What runs on one Mac

All of it, up to the point where real hardware differences matter.

- Each guest is 512 MiB, so eight guests is 4 GiB -- comfortable.
- Under HVF they run on the real cores at native speed, which matters
  when eight of them are running.
- Under TCG the whole cluster is deterministic, which is what makes a
  disagreement between two runs a bug rather than a race. Expect it to be
  slow with eight guests.
- `mlos run` grows a way to start N guests and the relay between them.
  Naming and addressing stay trivial: nodes are numbered, and a node
  learns its own number from a boot argument, the way it already learns
  its console and its script.

What one Mac cannot show: real PCIe, real NIC behaviour, real
heterogeneity of storage, and any effect that depends on genuinely
different hardware. Those need the second machine, and they are what M8
is for.

## 6. Order of work

1. **Multi-node `mlos-sim`** and a cost graph. Answers all three
   questions in software, with modelled costs. No emulator.
2. **A second virtio-serial port and a host relay.** Two guests exchange
   objects. The protocol gets designed here.
3. **A remote provider in the kernel**, so `acquire` can be satisfied
   from a peer and the M3 policy governs the choice.
4. **Homogeneous cluster measurements**, 2 then 4 then 8 guests.
5. **Heterogeneous measurements**, one variable at a time.
6. **`virtio-net`**, and the same measurements across two machines.

Steps 1 to 5 run on the Mac. Step 6 is where the second machine arrives,
and it is the same step that makes
[external-asks.md](external-asks.md)'s "a second machine" ask real.

## 7. What would make this fail honestly

If a cluster-aware policy does not beat independent local policies on a
homogeneous cluster, the distributed idea is not carrying its weight and
the project should say so and stop at one node. That is the same
discipline M3 step 006 applies to known-next-use, and for the same
reason: a measurement that cannot come out negative is not a measurement.
