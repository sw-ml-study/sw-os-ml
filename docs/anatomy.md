# Anatomy

How MLOS is built, what it does, how it does it, and what it does not
do. Four sections, each a paragraph a stranger can read in a minute,
with the detail one link away. [dream.md](dream.md) is why;
[status.md](status.md) is the ground truth for every claim here.

## How it is built

From scratch in Rust, as forty-odd small crates, each owning one concern
and kept small by a size gate: 25 lines a function, four functions a
module, four modules a crate. The crate list is the architecture, and a
reader can hold it in one screen ([design.md s.2](design.md#2-crate-layout)).
The kernel is `no_std`; `unsafe` is confined to the hardware layer and
the drivers, every block naming the invariant it relies on. Everything
above the hardware layer is architecture-neutral and is built for both
targets on every run, which is why the x86-64 port needed zero new
driver code. Every policy runs first on the host, in a simulator that
links the same crates, against a recorded trace, and the kernel must
reproduce the simulator's counts to the integer before a number is
quoted. Development is stepwise sagas in parallel lanes, each commit a
design record; there is no CI, because the local gate is stricter than
any runner was ([AGENTS.md](../AGENTS.md)).

Each crate's design reasoning, and the lessons its bugs taught, live in
one note per crate under [notes/](notes/README.md); the code keeps the
invariant and a link. That is a rule ([AGENTS.md](../AGENTS.md),
"Comments: the invariant in the code, the reasoning in the notes"), and
this document plus [dream.md](dream.md) are where the reasoning above
the crate level lives.

The layers, bottom up:

| Layer | Crates | What it owns |
| --- | --- | --- |
| Hardware | `mlos-hal`, `mlos-hal-aarch64`, `mlos-hal-x86-64`, `mlos-mmu-*`, `mlos-trap-*`, `mlos-gic-aarch64`, `mlos-apic-x86-64` | Entry, identity map, exceptions, interrupts, a clock |
| Devices | `mlos-virtio`, `mlos-virtio-blk`, `mlos-virtio-console`, `mlos-pl011`, `mlos-uart16550`, `mlos-fdt`, `mlos-pvh` | Discovery from the device tree or the PVH table; the block device the model lives on; consoles |
| Objects | `mlos-abi`, `mlos-objtab`, `mlos-provider`, `mlos-arena`, `mlos-stream`, `mlos-objman` | Identity and metadata, the table, providers and tiers, the arena, the declared stream, the fault path |
| Policy | `mlos-policy` | Demand, FIFO, LRU, known-next-use: pure, `no_std`, shared with the simulator |
| Observation | `mlos-metrics`, `mlos-events`, `mlos-layout`, `mlos-snapshot`, `mlos-spaces` | Counters, the residency event ring, the layout documents a visualizer draws |
| Shell | `mlsh`, `mlos-line`, `mlos-queue` | The in-guest inspector: `model`, `get`, `sweep`, `replay`, `arena`, `objs` |
| Host tools | `mlos-cli`, `mlos-sim`, `mlos-trace`, `mlos-workload`, `mlos-synth`, `mlos-image-map` | Build and boot, the simulator, traces and workloads, the synthetic model |

## What it does

Boots to a shell as a guest on Apple Silicon and on x86-64, from one
source tree. Registers a model's tensors as kernel objects across three
tiers with real, different costs. Faults a missing object in from a
virtio block device and counts the fault by class. Is told, by a system
call, the order a workload will touch its objects, and writes each
object's next use into the table. When memory is full, chooses what to
evict by a policy that reads that field, and gives the bytes back.
Replays a recorded workload under any of four policies and reports what
it cost. Emits layout and event documents a visualizer can draw. All of
it identically on two architectures, and identically to the host-side
simulator, to the integer. [status.md](status.md#what-works-today) has
the row-by-row list.

## How it does it

An object is `class / model / layer / tensor / tile`
([design.md s.4](design.md#4-the-ml-object-table)), with metadata saying
where it lives, what it costs to reload or recompute, how often it has
been used and when it is next wanted. Objects sit in an open-addressed
table in the kernel, because hits are the common case and must not cross
a boundary: in the measured band 86 to 95 percent of accesses are hits.
A miss becomes a `MODEL_FAULT` naming the layer, not an address
([architecture.md s.4](architecture.md#4-the-model-fault)); a provider
services it; a first-fit arena with a coalescing free list places it. A
declared stream and a cursor supply `next_use`
([architecture.md s.3.5](architecture.md#35-stream)). A policy sees the
resident set through a narrow view and names one victim at a time until
the object fits ([architecture.md s.5](architecture.md#5-residency-policy-where-the-knowledge-pays)).
The simulator is the kernel's object manager without a machine under it,
running the same policy and arena crates, which is what makes the two
comparable and what made their disagreements findable: four real bugs
came out of the last thirteen reads of difference
([status.md](status.md#the-kernel-and-the-simulator-agree)).

## What it does not do

Two lists, kept apart on purpose. "Not yet" is a different claim from
"not ever".

**Not ever, by design** ([PRD.md s.7](PRD.md#7-explicit-non-goals)):

- Run on bare metal. It is a guest; the hypervisor is the hardware.
- Own a filesystem, a network stack or a GPU driver. The host does, and
  MLOS reaches them through narrow virtio interfaces
  ([architecture.md s.7.4](architecture.md#74-the-hostguest-boundary-what-mlos-does-not-own)).
- Be Linux, or run Linux programs. It is a new kernel with a new
  abstraction, not a compatible one.
- Reimplement an inference framework. It decides where state lives; it
  does not compute with it.

**Not yet, and the plan says when:**

- Run a model. There is no arithmetic anywhere; the traces are shaped by
  a real checkpoint's tensor inventory and the transformer's forward
  pass, not recorded from an engine. Closing that is M4 or later.
- Host anything. No processes, no scheduler beyond one thread; the shell
  runs in the kernel. Userspace and sessions arrive with M4.
- Share one read among sessions (M4), or degrade under pressure instead
  of refusing (M5).
- Place objects in real host memory, storage or GPU memory (M6), or in
  another machine (M7).
- Boot under Virtualization.framework with a console, which needs
  virtio-pci (M6).
