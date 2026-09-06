# MLOS -- Product Requirements

Status: draft, pre-implementation.
Owner: Software Wrighter LLC.
Companion documents: [architecture.md](architecture.md),
[design.md](design.md), [plan.md](plan.md), [status.md](status.md).

---

## 1. The one-sentence thesis

> Operating systems virtualize scarce resources whose access patterns
> they cannot predict. Machine-learning workloads invert that
> assumption -- most of their state is immutable, their access order is
> known in advance, some of it is cheaper to recompute than to store,
> and many clients want the *same* bytes at the *same* time. An OS
> built on those inverted assumptions can manage model state better
> than a page-based OS with an ML framework bolted on top.

MLOS is that OS. It is a new kernel, not a Linux or BSD derivative.

## 2. The problem

Today the stack looks like this:

```
  inference framework   vLLM / llama.cpp / TensorRT-LLM
        |                 knows the future access order
        |                 knows what is recomputable
        |                 knows which sessions share weights
        v
  operating system      Linux
                          knows none of that
                          sees anonymous pages and a GPU chardev
                          makes eviction decisions blind
```

Every framework independently rebuilds the same machinery: a KV block
allocator, a weight cache, an offload policy, a prefetcher, an
admission controller. Each one does it inside a single process, so none
of them can arbitrate between *each other*. Two inference servers on
one box do not share a weight stream, do not share a residency budget,
and cannot cooperatively degrade under memory pressure. The kernel,
which is the only component positioned to arbitrate, has been told
nothing it could arbitrate with.

Meanwhile the resource economics moved. CPU cycles and cheap storage
are abundant; HBM and DRAM are the scarce, expensive resource. The OS
abstraction that matters is no longer "which process gets the CPU" but
"which model state gets to be resident, and who gets to share it."

### The specific failures we are targeting

1. **Blind eviction.** Linux evicts by recency. A transformer knows the
   exact next-use distance of every weight tile. Belady's optimal
   algorithm is impossible in general and nearly free here, and no
   production OS exploits it.
2. **Duplicated residency.** Three processes serving the same model
   hold three copies, or share by luck via the page cache. Nothing in
   the system understands "these are the same weights."
3. **Process-major execution.** Each session drags the whole model past
   the memory system. Parameter-major execution -- read layer 13 once,
   apply it to every waiting session -- is a scheduling inversion no
   process scheduler can express.
4. **OOM instead of degradation.** An ML workload can be made
   *mathematically cheaper*: quantize the cold KV, shrink the context,
   drop RAG candidates, recompute instead of store. An ordinary OS
   under pressure can only kill something.
5. **Opaque accounting.** "This process used 40 GB" is useless. The
   answer that matters is bytes-resident per useful token, and which
   ML object class they went to.

## 3. What MLOS is

A microkernel whose first-class objects are ML objects, with a
model-object manager where a conventional kernel has a VM subsystem.

```
+---------------------------------------------------------------+
|  inference session   |  embedding worker  |  policy simulator  |
+---------------------------------------------------------------+
|  ML runtime (userspace): tensor ops, model loaders             |
+---------------------------------------------------------------+
|  Model Object Manager    | object table | residency | leases   |
|                          | prefetcher   | eviction  | streams  |
+---------------------------------------------------------------+
|  MLOS microkernel: scheduling, IPC, capabilities, accounting   |
+---------------------------------------------------------------+
|  Providers: DRAM | NVMe | GPU VRAM | host file | recompute      |
+---------------------------------------------------------------+
|  HAL: aarch64 / x86-64, PCIe, virtio, GPU, (later) FPGA ML-MMU |
+---------------------------------------------------------------+
```

The five kernel concepts, spelled out in
[architecture.md](architecture.md#3-the-five-kernel-concepts):
`ML_OBJECT`, `PROVIDER`, `TIER`, `LEASE`, `STREAM`.

## 4. Who it is for

| User | What they get | Why they cannot get it today |
| --- | --- | --- |
| **OS researcher** | A running system where semantic paging can be measured against page-based paging on identical traces | No such system exists; framework offload is not OS policy |
| **ML systems engineer** | Multi-tenant residency arbitration across processes, and parameter-major scheduling | Frameworks arbitrate only within one process |
| **Hardware researcher** | A concrete, emulated ML-MMU contract to build gateware against | ML memory management has no ISA-level abstraction to target |
| **The author (primary)** | The systems half of a research programme whose other halves are emufpga (streaming hardware), demo-memory (policies) and sw-mlpl (the language) | -- |

This is a research and teaching operating system. It is not competing
with Linux for production inference serving, and success does not
require that it ever does.

## 5. Success criteria

### 5.1 The proof-of-concept gate (the thing we are building toward)

MLOS is a success at PoC when, booted as a guest under a hypervisor on
Apple Silicon and on x86-64:

- **G1 -- It boots.** MLOS starts from firmware, initializes memory,
  brings up a timer and a console, and reaches a userspace shell. On
  `aarch64` under QEMU/HVF and on `x86_64` under QEMU/KVM, from the
  same kernel source.
- **G2 -- It holds an object table.** A synthetic transformer's weights
  are registered as `ML_OBJECT`s across at least three tiers (RAM,
  block store, recompute) and resolved through providers.
- **G3 -- It faults.** Touching a non-resident object raises a
  `MODEL_FAULT` carrying `{class, model, layer, tile}`, the provider
  layer services it, and the fault is counted per class.
- **G4 -- It beats LRU with knowledge.** On a recorded layer-access
  trace, known-next-use replacement demonstrably reduces provider reads
  against LRU and FIFO under the same residency budget. The numbers are
  measured, not asserted.
- **G5 -- It shares a stream.** Four concurrent sessions needing the
  same layer cause *one* provider read, not four. Parameter-major
  scheduling is demonstrated against a process-major baseline on the
  same trace.
- **G6 -- It degrades instead of dying.** Under a shrinking residency
  budget the system walks a declared degradation ladder (quantize cold
  KV, shrink context, drop candidates) and reports the quality/latency
  it delivered, rather than OOM-killing a session.
- **G7 -- It touches a real GPU.** On the Linux/NVIDIA host, MLOS as a
  guest reaches a passed-through GPU over PCIe far enough to enumerate
  it, map its BARs, and move a tensor into device memory under object
  manager control. Full compute is a stretch goal; *managed placement
  across the PCIe boundary* is the gate.
- **G8 -- The ML-MMU is emulated.** The FPGA ML-MMU register contract
  exists, is implemented as an emulated device, and the kernel uses it
  through the same provider interface it will use for real gateware.

G1--G6 are achievable on the Apple Silicon machine alone. G7 requires
the Linux/NVIDIA host. G8 is software-only until emufpga catches up.

### 5.2 The metrics MLOS reports

A conventional OS reports throughput and residency. MLOS reports the
ratios that matter when RAM is the expensive resource. These come from
the SPM work in `docs/research.txt` s.25 and are elevated to
system-level counters:

| Metric | Definition |
| --- | --- |
| `Rm` | resident ML bytes / total model bytes |
| `Ps` | useful parameter applications / parameter values read |
| `Bt` | backing-store bytes / generated token |
| `Rc` | recomputed byte-equivalents / token |
| `Pf` | model faults / token, broken down by object class |
| `Ph` | prefetch hit rate |
| `Ks` | KV bytes / active session |
| `Ss` | sessions / GB of resident budget |

`Ss` and `Ps` are the headline numbers. The optimization objective is
**useful work per resident byte**, not tokens per second.

## 6. Requirements

### 6.1 Functional

- **F1** Register, resolve, acquire, release and prefetch typed ML
  objects through a stable syscall surface.
- **F2** Providers for: resident DRAM, block store, host file (via
  virtio), GPU device memory, recompute, and the emulated ML-MMU.
- **F3** Pluggable residency policy: demand, FIFO, LRU,
  known-next-use, cost-aware (reload vs recompute vs compress), and
  MoE router-probability prefetch.
- **F4** Session objects carrying a residency budget, a priority and a
  quality contract; admission control that refuses or degrades a
  session rather than overcommitting.
- **F5** Parameter-major scheduling: the schedulable unit is
  `(model-state chunk, set of sessions)`.
- **F6** Tensor-handle IPC: messages transfer ownership of an object
  and a lease, not bytes.
- **F7** Per-class accounting for every metric in s.5.2, readable from
  userspace and dumpable as a trace.
- **F8** Capability-based access to devices and objects. No ambient
  authority over a GPU or a model.

### 6.2 Non-functional

- **N1** Rust 2024, `no_std` kernel. `unsafe` confined to the HAL and
  driver crates, each block carrying a `SAFETY:` note.
- **N2** Boots as a guest under at least two hypervisors per
  architecture, so no single vendor's quirks become load-bearing.
- **N3** Every policy runs against a recorded trace in a host-side
  simulator, deterministically and without a VM, before it enters the
  kernel. A policy that cannot be replayed cannot be committed.
- **N4** `sw-checklist` at zero failures and zero warnings, from the
  first commit onward.
- **N5** No POSIX compatibility surface. We are not writing a libc.
  Where a Unix-shaped interface would help, it is a userspace library
  over MLOS primitives, never a kernel obligation.

## 7. Explicit non-goals

These are stated so they cannot creep in:

1. **Bare-metal boot.** MLOS boots in a VM. Real hardware bring-up is
   a different project with a different cost structure, and it buys us
   nothing that the hypervisor does not already give us.
2. **Being a Linux derivative, or source/binary compatible with one.**
3. **A POSIX personality, a libc, or an ELF loader for Linux binaries.**
4. **Training.** Inference first; LoRA-scale mutable state second;
   gradients, optimizer state and checkpoints are parked
   (`docs/research.txt` s.31).
5. **Writing a general-purpose GPU driver.** See
   [architecture.md](architecture.md#7-the-gpu-problem-stated-honestly)
   -- we consume paravirtual or GSP-mediated interfaces, and we do not
   pretend a from-scratch NVIDIA or Apple GPU driver is in scope.
6. **The AI/agent operating environment.** Durable agent jobs, virtual
   context, tools-as-capabilities, agent fork/join: a sibling project
   sharing the substrate (`docs/research.txt` s.1--34), deliberately
   not this one. MLOS virtualizes the *model*; that system virtualizes
   the *agent*.
7. **Competitive inference performance.** We measure ratios, not
   records. If MLOS is slower than vLLM in absolute tokens/sec while
   serving more sessions per resident GB, that is the result we wanted.
8. **A RISC-V first target.** RISC-V is the ISA we would prefer; GPU
   support there is not mature enough to be the *first* target. The
   HAL keeps the door open (see
   [architecture.md](architecture.md#6-isa-choice-and-why-not-risc-v-yet)).

## 8. Constraints we did not choose

- **The MMU virtualizes pages, not tensors.** No shipping CPU has an
  ML-MMU. Every semantic mapping MLOS performs is software over page
  hardware, until an FPGA card provides otherwise. The architecture is
  therefore written so the ML-MMU is a *provider*, swappable for
  gateware without touching the object abstraction.
- **GPUs are opaque.** Their programming interfaces are vendor
  firmware contracts, not documented ISAs. MLOS meets them where they
  are already virtualized.
- **Apple Silicon cannot pass through its GPU.** It is on-die, with no
  IOMMU path to a guest. GPU work on the Mac goes through a
  paravirtual device to the host's Metal stack.
- **The development machine is a Mac; the GPUs are on Linux.** The
  repo must be equally workable in both places, cloned rather than
  ported.

## 9. Open questions

Carried into [architecture.md](architecture.md) and revisited at each
milestone:

- **Q1** Is the object granularity a weight *tile* or a whole tensor?
  Tiles give better residency control and a larger table. Start with
  tensor-granular objects and a tile sub-index; revisit after G4.
- **Q2** Does the model object table belong in the kernel or in a
  privileged service? Microkernel purity says service; fault latency
  says kernel. Provisional answer: table in kernel, *policy* in a
  service, with the policy consulted on eviction and prefetch only.
- **Q3** How much of the GPU does the guest need to own for G7 to be
  meaningful? Enumerate-and-map is the floor; a submitted kernel is
  the ceiling.
- **Q4** Does the degradation ladder need a quality oracle in-kernel,
  or can quality be a userspace-declared contract the kernel merely
  accounts against? Provisional: contract only.
