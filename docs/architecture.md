# MLOS -- Architecture

Status: draft, pre-implementation.
Read [PRD.md](PRD.md) first. Implementation detail lives in
[design.md](design.md).

---

## 1. Architectural stance

MLOS is a **microkernel with a model-object manager where a
conventional kernel has a VM subsystem.**

Three commitments follow from that sentence, and everything else in
this document is a consequence of one of them:

1. **The kernel arbitrates; it does not compute.** No tensor math in
   the kernel. The kernel owns the object table, residency, leases,
   faults, capabilities and accounting. Matmuls happen in userspace or
   on an accelerator.
2. **Semantics live above the mechanism.** The kernel knows an object
   is a `KV_BLOCK` with a next-use distance and a recompute cost. It
   does not know what attention is. Policy that needs to know is a
   userspace service the kernel calls out to.
3. **Physical representation is not part of the abstraction.** An
   object's identity is `L17.attn.q_proj`; whether that is FP16 in
   DRAM, Q4 on NVMe, or a stream from an FPGA is a placement decision
   the kernel makes and can change while a consumer holds a handle.

Commitment 3 is the one that justifies the word "operating system."
Virtual memory's value was never that disks could impersonate RAM; it
was that the abstraction presented to software stopped naming the
physical resource. That is the move being repeated here, one level up.

## 2. What inverts, and why it matters

| Assumption in a page-based OS | What ML actually does | What MLOS can therefore do |
| --- | --- | --- |
| Future references are unknowable | Layer order is known; MoE router emits a probability distribution over experts | Belady-optimal replacement; probabilistic prefetch |
| Memory contents are unique per process | Weights are immutable and identical across sessions | One physical copy, N leases; one read serves N sessions |
| Lost state must be restored | Activations are often cheaper to recompute than to reload | Eviction chooses `DISCARD+RECOMPUTE` over `STORE` |
| Work has a fixed memory cost | KV can be quantized, context truncated, candidates dropped | Degrade under pressure instead of OOM-killing |
| The schedulable unit is a process | Many sessions want the same bytes at the same time | Schedule `(model-state chunk, session set)` |
| Access is byte-addressed and random | Parameters are consumed sequentially, once per pass | Stream, never resident |

Every one of those rows is an OS mechanism that does not currently
exist because no OS was told enough to build it.

## 3. The five kernel concepts

```
        ML_OBJECT ----- has many ----> LEASE  (who needs it, how long)
            |
            | resolved by
            v
        PROVIDER  ----- places into --> TIER   (where it currently is)
            ^
            | supplies
            |
          STREAM   (a declared future access sequence)
```

### 3.1 `ML_OBJECT`

A named region of model state, with a class and metadata. The class
determines which policies apply.

```
MODEL
 +-- LAYER
 |    +-- WEIGHT_TILE      immutable, huge, sequential, shareable
 |    +-- SCALE            tiny, hot, always resident
 |    +-- EXPERT           immutable, selected probabilistically
 +-- KV_BLOCK              mutable, per-session, grows with context
 +-- ACTIVATION            transient, recomputable
 +-- EMBED_BLOCK           immutable, random access
 +-- RAG_BLOCK             immutable, semantically addressed
 +-- ADAPTER               small, mutable at LoRA scale
```

Metadata carried per object, and what each field is *for*:

| Field | Used by |
| --- | --- |
| `size`, `precision` | admission control, budget accounting |
| `tier`, `provider` | resolution |
| `next_use` | known-next-use replacement (s.5) |
| `reuse_count` | frequency-aware expert caching |
| `reload_cost` | eviction cost model |
| `recompute_cost` | eviction cost model; `NONE` for weights |
| `share_count` | parameter-major scheduling (s.6) |
| `mutability` | whether a copy-on-write overlay is needed |
| `owner` | capability checks and per-session accounting |

`next_use` is the field that does not exist in any page-based OS, and
it is the one that makes the whole design worth building.

### 3.2 `PROVIDER`

Where an object can come from. Providers are the only place that knows
about physical media.

```
resolve(object_id)            -> placement
read(object_id, off, len)     -> bytes, or DMA descriptor
prefetch(object_id, hint)     -> void
release(object_id)            -> void
cost(object_id)               -> {latency, bandwidth, energy}
```

Initial provider set: `dram`, `blockstore`, `hostfile` (virtio-fs /
virtio-blk to the host), `device` (GPU VRAM across PCIe), `recompute`
(re-derive rather than fetch), `mlmmu` (the emulated FPGA card).

`cost()` is what makes eviction decidable. An eviction policy that
cannot ask "what would getting this back cost me" is guessing.

### 3.3 `TIER`

A residency class, ordered by cost of access:

```
HOT     device memory / pinned DRAM      leases held, never evicted silently
WARM    DRAM                             evictable under pressure
COLD    NVMe / block store               fetched on fault
STREAM  never resident                   consumed as it passes
ARCHIVE recompute, regenerate, or drop   no bytes stored at all
```

`STREAM` is the tier a page-based OS cannot express. A streamed object
is never "in memory" in a way you could point at; it is a scheduled
flow that compute is arranged around. This is the emufpga Serial
Parameter Machine result promoted to an OS tier.

### 3.4 `LEASE`

A time-bounded claim on an object by a session. Leases, not reference
counts, because a lease carries *intent*:

```
LEASE { session, object, class, deadline, exclusivity }
   PIN         resident until released; counts against session budget
   BORROW      resident for one operation; may be revoked between ops
   STREAMING   the holder will consume it in order, once
   SPECULATIVE prefetched on a guess; free to drop
```

A `SPECULATIVE` lease that is never upgraded is a prefetch miss, and
that is how `Ph` gets counted.

### 3.5 `STREAM`

A declared future access sequence: "this session will touch L0..L31 in
order, twice." A stream is the mechanism by which the kernel learns
`next_use` without inferring it. Registering a stream is how a
userspace runtime tells the kernel the truth it already knows.

Two stream kinds:
- **Deterministic** -- dense layer sweep. Exact next-use distances.
- **Probabilistic** -- MoE routing. A distribution over experts, updated
  per token by the router. Feeds prefetch, not eviction certainty.

## 4. The model fault

The central mechanism. A conventional fault says *an address was not
mapped*. A model fault says *a named piece of model state was not
resident*, and carries enough to make a good decision.

```
   userspace: acquire(L17.attn.q_proj, PIN)
        |
        v
   object table lookup ------ resident? ----> return handle   (fast path)
        |
        | not resident
        v
   MODEL_FAULT { class: WEIGHT_TILE, model: 7, layer: 17,
                 tile: 3, session: 4, deadline: t+2ms }
        |
        +--> policy service consulted (which provider? evict what?)
        |
        +--> provider.read()  or  provider.recompute()
        |
        +--> place into tier, install lease, account the fault
        v
   resume
```

Faults are counted per class (`Pf`), because "40,000 faults" is
meaningless and "38,000 of them were cold KV blocks" is a diagnosis.

The fast path must not cross into a service. That is the practical
reason the object *table* is in the kernel (open question Q2 in the
PRD): a fault that costs an IPC round trip before it even knows where
to look is a fault too expensive to have.

## 5. Residency policy: where the knowledge pays

Policies are pluggable and all of them run against recorded traces in
a host-side simulator before they enter the kernel.

| Policy | Basis | Applies to |
| --- | --- | --- |
| `demand` | nothing; load on use | baseline |
| `fifo` / `lru` | recency | baseline |
| `next_use` | declared stream position | dense weights, KV |
| `cost_aware` | `min(reload_cost, recompute_cost)` vs size | activations |
| `router_prefetch` | MoE probability distribution | experts |
| `frequency` | `reuse_count` | experts across requests |
| `attention_score` | userspace-supplied importance | KV blocks |

**`next_use` is the headline experiment.** Belady's algorithm is the
provably optimal replacement policy and is normally unimplementable
because it requires knowing the future. A dense transformer *hands you
the future*: after L3 comes L4, and L3 is next needed one full pass
from now. Demonstrating measurable separation between `lru` and
`next_use` on a real trace is PoC gate G4, and it is the cleanest
possible statement of why an ML-aware OS is not just a framework with
different packaging.

### Eviction as a cost decision, not a recency decision

```
   activation A:  2 MB,  recompute 200us,  NVMe reload 700us  -> DISCARD
   expert  E7 : 40 MB,  recompute  n/a  ,  NVMe reload   3ms  -> DEMOTE
   cold KV    : 12 MB,  recompute  n/a  ,  compressible 4:1   -> QUANTIZE
```

Four eviction verbs, not one: `DEMOTE`, `DISCARD` (recompute later),
`QUANTIZE`, `DROP`.

### Degradation ladder

Under budget pressure the kernel walks a declared ladder rather than
killing a session:

```
L0  full precision KV, full context
L1  cold KV -> Q8
L2  cold KV -> Q4
L3  summarize oldest context span
L4  reduce RAG candidate count
L5  truncate context window
L6  refuse new sessions (admission control)
L7  terminate lowest-priority session      <- last resort, not first
```

Each session declares a contract (`quality >= x`, `latency <= y`,
`resident <= z bytes`) at admission; the kernel accounts against it and
reports what it actually delivered.

## 6. Scheduling: the parameter-major inversion

A conventional scheduler is process-major:

```
for session in sessions:          # each session drags the model past memory
    for layer in model:
        read(layer); compute(session, layer)
```

MLOS can be parameter-major, because it knows which sessions want which
objects:

```
for layer in model:               # the model passes memory once
    read(layer)
    for session in sessions_waiting_on(layer):
        compute(session, layer)
```

The schedulable unit becomes `(model-state chunk, set of sessions)`.
Four sessions on the same layer cause one provider read, not four --
PoC gate G5. This is the storage-side equivalent of SIMD, and it is the
direct continuation of the SPM scan-productivity result (`Ps`) from
emufpga.

The cost is latency variance: a session must wait for its layer's turn.
So the scheduler is two-dimensional -- it allocates *compute* and
*residency* simultaneously, and a latency-contracted session can buy
its way out of the batch at the price of a private read.

## 7. The GPU problem, stated honestly

This is where a from-scratch OS meets reality, so it gets stated
plainly rather than optimistically.

**Writing a GPU driver from scratch is not in scope.** A modern GPU's
programming interface is a vendor firmware contract of enormous
surface area, not a documented ISA. MLOS therefore consumes GPUs
through interfaces that are *already virtualized*, and the object
manager's job is placement and accounting across the PCIe boundary,
not shader compilation.

Three tiers of GPU ambition, in increasing order of cost:

| Tier | What MLOS does | Where it works |
| --- | --- | --- |
| **A. Managed placement** | Enumerate the device, map BARs, DMA tensors in and out under object-manager control. Compute happens elsewhere or not at all. | Linux/NVIDIA host with VFIO passthrough |
| **B. Paravirtual compute** | Guest driver speaks a virtio protocol; the host executes the work on the real GPU | Apple Silicon (virtio-gpu/Venus -> MoltenVK -> Metal); Linux (virtio-gpu native context) |
| **C. Native driver** | MLOS drives the hardware itself | Only plausible via NVIDIA's GSP path (s.7.2), and only as a stretch goal |

**Tier A is PoC gate G7.** Tier B is the realistic route to actually
running a model. Tier C is a research question, not a plan.

### 7.1 Why Apple Silicon cannot pass through its GPU

The Apple GPU is on-die and unified with system memory. There is no
discrete PCIe function to hand to a guest, and no IOMMU path that would
let one. Apple's own answer is paravirtualization: the guest gets a
virtual graphics device and submits Metal work that the host stack
executes on the real GPU. Benchmarks put a macOS guest around 92% of
host Metal performance, but the paravirtual device advertises a
reduced feature set -- roughly an Apple-5-family device, 32 KB max
threadgroup memory, no SIMD-group matrix support -- which is precisely
the feature modern ML kernels want. ([eclecticlight.co](https://eclecticlight.co/2023/10/26/how-good-is-gpu-access-for-apple-silicon-virtual-machines/), [trycua/cua](https://github.com/trycua/cua/blob/main/blog/gpu-passthrough-macos-vms.md))

That path is also macOS-guest-only. For a *custom* guest OS the
relevant stack is the libkrun one:

```
   MLOS guest
     virtio-gpu driver, Venus protocol   <-- what MLOS would implement
        |
   libkrun VMM (Hypervisor.framework)
        |
   virglrenderer  ->  MoltenVK  ->  Metal  ->  Apple GPU
```

libkrun/krunkit is a Red Hat VMM that links Hypervisor.framework
directly rather than Virtualization.framework, boots EFI guests, and
exposes virtio-gpu with the Venus (Vulkan) protocol; the host decodes
Venus and forwards to MoltenVK. It has been measured at ~40x the
software path for container GPU compute, and is the production route
for llama.cpp on Apple Silicon in containers. ([sinrega.org](https://sinrega.org/2024-03-06-enabling-containers-gpu-macos/), [Red Hat Developer](https://developers.redhat.com/articles/2025/09/18/reach-native-speed-macos-llamacpp-container-inference))

**Consequence for MLOS:** on Apple Silicon, "GPU" means implementing a
virtio-gpu/Venus guest driver. That is a large but *bounded* piece of
work against a documented protocol -- unlike an Apple GPU driver, which
is not.

### 7.2 What is actually possible on NVIDIA

Since the GSP (GPU System Processor) generation, the great majority of
NVIDIA's resource manager runs as firmware *on the GPU*, and the host
kernel module is largely an RPC shim over it. NVIDIA's own open kernel
modules are built this way, and the architecture means only a thin,
per-OS RPC layer needs porting. ([open-gpu-kernel-modules analysis](https://eunomia.dev/zh/blog/posts/nvidia-open-driver-analysis/), [DeepWiki](https://deepwiki.com/NVIDIA/open-gpu-kernel-modules))

That is the *only* reason Tier C is even mentionable. It remains a
stretch goal: "thin" is relative, the RPC surface is large, and no
hobby OS has publicly done it. The plan does not depend on it.

### 7.3 GPU virtualization options, compared

| Mechanism | Guest sees | Isolation | Good for MLOS? |
| --- | --- | --- | --- |
| **VFIO passthrough** | The real PCIe device | IOMMU-enforced, whole device | **Yes -- the G7 path.** Simplest model, hardest to share |
| **NVIDIA vGPU** | A virtual GPU, full NVIDIA guest driver | Software time-slicing | No -- requires an NVIDIA guest driver we do not have |
| **MIG** | A hardware partition | Silicon-level, dedicated memory/bandwidth | Interesting later for multi-tenant residency; needs A100/H100/RTX-PRO class |
| **SR-IOV (AMD)** | A virtual function | Hardware | Alternative if the NVIDIA path stalls |
| **virtio-gpu native ctx / Venus** | A paravirtual device | VMM-mediated | **Yes -- the compute path**, on both hosts |

VFIO requirements that shape the hardware we buy: VT-d/AMD-Vi enabled,
the GPU alone in its IOMMU group (with only its audio function and
bridges), the host NVIDIA driver kept away from it via early
`vfio-pci` binding, and Above-4G decoding plus resizable BAR enabled in
firmware. ([Level1Techs](https://forum.level1techs.com/t/guide-host-setup-for-qemu-kvm-gpu-passthrough-with-vfio-on-linux/235973), [cloudrift.ai](https://www.cloudrift.ai/blog/host-setup-for-qemu-kvm-gpu-passthrough-with-vfio-on-linux))

**Resizable BAR matters more to MLOS than to a gaming VM.** With ReBAR,
the whole of VRAM is mapped into the host/guest address space rather
than a 256 MB window, which is exactly what an object manager doing
placement across the PCIe boundary needs.

## 8. Host and boot environment

MLOS never boots on bare metal. The question is which hypervisor, and
the answer differs per host.

### 8.1 Apple Silicon: what can boot a custom OS

macOS exposes virtualization at two levels, and the difference decides
everything:

- **`Hypervisor.framework`** -- the low-level EL2 interface. You build
  the VMM. Full control over memory layout, devices, entry state.
- **`Virtualization.framework`** -- the high-level, opinionated layer
  built on it. Handles devices, but constrains the boot path.

Virtualization.framework will boot a custom OS via `VZEFIBootLoader`
(macOS 13+), which gives the guest a UEFI environment and boots from an
ISO or raw disk image. It is genuinely usable for a from-scratch OS,
with two sharp edges: it wants an **uncompressed raw arm64 image** (a
compressed EFI zboot kernel fails with an unhelpful internal error,
because no firmware is present to decompress it), and its device set is
whatever Apple provides -- you cannot add one. ([Apple Developer Forums](https://developer.apple.com/forums/thread/731074), [Code-Hex/vz](https://pkg.go.dev/github.com/Code-Hex/vz/v3))

Nested virtualization arrived in macOS 15 on M3 and later, surfaced as
`nestedVirtualizationSupported` / `nestedVirtualizationEnabled` on
`VZGenericPlatformConfiguration`. It matters here only if we want to
run the MLOS VM inside a Linux VM on the Mac; it is not on the critical
path. ([Parallels forum](https://forum.parallels.com/threads/macos-15-sequoia-nested-virtualization-for-m3-macs.364397/))

Note that QEMU-inside-a-macOS-guest cannot use HVF -- nested HVF is not
supported, and QEMU falls back to TCG. ([qemu issue 2981](https://gitlab.com/qemu-project/qemu/-/issues/2981))

**The options, ranked for our purpose:**

| Option | Boot path | Devices | Verdict |
| --- | --- | --- | --- |
| **QEMU + HVF, `virt` machine, EDK2 AAVMF** | UEFI, or `-kernel` direct | Anything QEMU emulates; add your own | **Primary dev target.** Maximum device flexibility, `gdb` stub, deterministic TCG fallback for CI |
| **libkrun / krunkit** | EFI, Hypervisor.framework directly | virtio incl. GPU/Venus | **The GPU path.** Adopt once a virtio-gpu guest driver exists |
| **Virtualization.framework (VZEFIBootLoader)** | UEFI, raw image | Apple's fixed set | Useful as a *second* hypervisor for N2, not primary |
| **UTM** | wraps both | -- | Convenience GUI; not a build target |

**Decision: QEMU + HVF on `-M virt` is the Apple Silicon development
target.** The reason is not performance -- it is that OS development
needs a debugger stub, custom devices (we need to emulate an ML-MMU),
and reproducible failure. QEMU gives all three; Virtualization.framework
gives none of them. HVF acceleration means aarch64 guests run at near
native speed, and we are writing an aarch64 guest, so no instruction
emulation is involved.

We keep Virtualization.framework working as a secondary target because
requirement N2 says no single hypervisor's quirks should become
load-bearing, and because it is the path a non-developer can run MLOS on.

### 8.2 Linux host: the GPU machine

**QEMU/KVM is the answer, and it is not close.** It is the only stack
that combines: VFIO device passthrough, arbitrary emulated devices (for
the ML-MMU), a gdb stub, and hugepage/CPU-pinning control.

| Alternative | Why not primary |
| --- | --- |
| **Cloud Hypervisor** | Rust, clean, fast, supports VFIO -- but a smaller device model and no easy path to a custom emulated device. Worth a second look for production-shaped runs |
| **Firecracker** | Deliberately minimal: no PCIe, therefore no GPU. Disqualified |
| **Xen** | PCI passthrough works, but the development loop is far worse |
| **VirtualBox / VMware** | No meaningful GPU passthrough for compute; not OS-dev tooling |

The Linux host doubles as the CI machine: `qemu-system-x86_64` and
`qemu-system-aarch64` under TCG give deterministic, hardware-free test
runs of the same kernel binaries.

### 8.3 The resulting development matrix

```
                Apple Silicon (M-series Mac)      Linux + NVIDIA
              +-------------------------------+ +-------------------------+
  primary     | QEMU + HVF, -M virt, aarch64  | | QEMU + KVM, q35, x86-64 |
  secondary   | Virtualization.framework      | | QEMU + KVM, virt,aarch64|
  GPU         | libkrun + virtio-gpu/Venus    | | VFIO passthrough (G7)   |
  CI          | QEMU TCG, both arches, no accel                          |
              +-------------------------------+ +-------------------------+
```

One kernel source tree, two architecture targets, four hypervisor
configurations. Nothing in the kernel above the HAL knows which one it
is running under.

## 9. ISA choice, and why not RISC-V yet

x86-64 is where the GPUs and the PCIe infrastructure are. Apple Silicon
is where the development machine is, and its unified memory makes the
residency questions *sharper*, not softer -- when weights, KV and
activations all contend for the same physical pool, placement decisions
have nowhere to hide.

RISC-V is the ISA this project would prefer on the merits: open, clean
privileged spec, and a plausible future home for an ML-MMU extension
that is not somebody's proprietary MSR. It is not the first target
because GPU support in the RISC-V ecosystem is not mature or widespread
enough for gate G7 to be reachable there.

The concession we make now to keep that door open: **the HAL is a
trait, not a set of `#[cfg]` blocks**, and the architecture-specific
crates are peers. Adding `mlos-hal-riscv64` must be an additive change,
never a refactor. Concretely, nothing above the HAL may name a page
size, a privilege level, an interrupt controller, or an atomics width.

## 10. The ML-MMU: what FPGA hardware would add

Today every semantic mapping is software over page hardware. The
architecture is written so that an ML-MMU is a *provider*, which means
gateware can replace software without the object abstraction changing.

### 10.1 What it would do

A conventional MMU maps `virtual address -> physical address`. An
ML-MMU maps a **model object address** to a placement:

```
   | model | layer | tensor | tile |   ->   { tier, provider, address, encoding }

   L0.WQ.3      NVMe    0x183000   Q4
   L0.WK.3      DRAM    0x020000   Q8
   L1.WQ.0      FLASH   0x810000   Q4
   KV.S3.B72    DRAM    0x040000   FP16
```

The translation is the least of it. The interesting functions are the
ones a CPU does badly and a stream engine does well:

| Function | Why it belongs in gateware |
| --- | --- |
| **Low-bit unpack / dequantize** | Q4/Q3/ternary unpacking is bit-slicing; a CPU spends most of its cycles shifting |
| **Descriptor-driven DMA** | Walk an object's tile list and stream it without per-tile CPU involvement |
| **Double-buffer control** | Fetch L[n+1] while compute consumes L[n], in hardware |
| **Gather / scatter** | KV block collection, expert selection |
| **Expert routing** | Take a router distribution, issue prefetches, in the datapath |
| **Checksum / integrity** | Free on a stream that is already passing through |
| **Next-use tracking** | Maintain the counters the eviction policy reads, without a fault |

That is a **tensor DMA engine with a translation table**, not a matrix
accelerator. It does not need enough gates to be a GPU -- which is
exactly why it is buildable on the FPGA hardware that already exists in
`../emufpga`.

### 10.2 The staged path

```
  Gen 0   emulated device in QEMU               <- this repo, PoC gate G8
  Gen 1   descriptor-driven DMA / stream engine
  Gen 2   + dequantizer
  Gen 3   + gather/scatter, next-use counters
  Gen 4   + expert router
  Gen 5   full model-memory coprocessor
```

Gen 0 is the deliverable here: a QEMU device model plus the register
contract, so the kernel's `mlmmu` provider is exercised long before any
gateware exists. Everything from Gen 1 onward is emufpga's work,
against the register contract this repo defines.

**The register contract is the actual artifact.** If MLOS and emufpga
agree on it, the software can be written now and the hardware can
arrive later without a rewrite. It is specified in
[design.md](design.md).

### 10.3 Where the card would sit

On the Linux host it is a PCIe card: a BAR with the translation table,
a descriptor ring, an MSI-X vector for stream completion. On Apple
Silicon there is no expansion slot, so the ML-MMU there is *always* the
emulated device -- which is another reason Gen 0 has to be good enough
to develop against on its own.

## 11. Relationship to the sibling projects

```
              demo-memory            emufpga           sw-mlpl
              (which bytes           (how to avoid     (the language
               matter)                storing them)     to express it)
                    \                    |                  /
                     \                   |                 /
                      +--------- sw-os-ml / MLOS ---------+
                           (who decides what stays,
                            where it lives, when it
                            moves, and who shares it)
```

- **demo-memory** is the policy laboratory. Eviction and retrieval
  policies get simulated there against traces before they become MLOS
  kernel policies.
- **emufpga** owns the streaming-hardware half. MLOS defines the
  ML-MMU register contract; emufpga builds gateware to it. The `Ps` and
  `Rp` metrics come from there.
- **sw-mlpl** is the eventual userspace: an array language is the
  natural shell for a system whose objects are tensors.
- The **AI/agent operating environment** is the sibling that
  virtualizes agents rather than models. Shared substrate, different
  semantics, explicitly out of scope here (PRD s.7.6).

## 12. Risks

| Risk | Consequence | Mitigation |
| --- | --- | --- |
| GPU work swallows the project | No OS gets written; we become a driver project | Gate G7 is *managed placement*, not compute. Tier C is explicitly a stretch goal |
| Object granularity chosen wrong | Table too large, or residency control too coarse | Start tensor-granular with a tile sub-index; revisit after G4 (PRD Q1) |
| Fault path too slow to be credible | The whole design reads as an academic toy | Table in kernel, policy in service, fast path never crosses IPC (s.4) |
| Apple paravirtual GPU feature gaps | No SIMD-group matrix means slow kernels | Accept it. The Mac is for OS semantics; the Linux box is for GPU truth |
| Synthetic traces prove nothing | G4/G5 results dismissed as constructed | Take traces from real models via emufpga's importers, not hand-written |
| Hypervisor lock-in | Apple or QEMU quirks become load-bearing | Requirement N2: two hypervisors per architecture, always |

---

## Sources

- [How good is GPU access for Apple silicon virtual machines? -- Eclectic Light](https://eclecticlight.co/2023/10/26/how-good-is-gpu-access-for-apple-silicon-virtual-machines/)
- [Virtualisation on Apple silicon Macs is different -- Eclectic Light](https://eclecticlight.co/2026/04/29/virtualisation-on-apple-silicon-macs-is-different/)
- [GPU passthrough on macOS VMs -- trycua/cua](https://github.com/trycua/cua/blob/main/blog/gpu-passthrough-macos-vms.md)
- [Enabling containers to access the GPU on macOS -- Sergio López](https://sinrega.org/2024-03-06-enabling-containers-gpu-macos/)
- [Reach native speed with macOS llama.cpp container inference -- Red Hat Developer](https://developers.redhat.com/articles/2025/09/18/reach-native-speed-macos-llamacpp-container-inference)
- [libkrun](https://github.com/libkrun/libkrun)
- [What you need to know to boot your own kernel -- Apple Developer Forums](https://developer.apple.com/forums/thread/731074)
- [Code-Hex/vz -- Virtualization.framework Go bindings](https://pkg.go.dev/github.com/Code-Hex/vz/v3)
- [macOS 15 Sequoia: nested virtualization for M3+ Macs -- Parallels](https://forum.parallels.com/threads/macos-15-sequoia-nested-virtualization-for-m3-macs.364397/)
- [QEMU issue 2981: hvf:tcg fallback](https://gitlab.com/qemu-project/qemu/-/issues/2981)
- [Guide: Host setup for QEMU/KVM GPU passthrough with VFIO -- Level1Techs](https://forum.level1techs.com/t/guide-host-setup-for-qemu-kvm-gpu-passthrough-with-vfio-on-linux/235973)
- [Host setup for QEMU KVM GPU passthrough with VFIO -- cloudrift.ai](https://www.cloudrift.ai/blog/host-setup-for-qemu-kvm-gpu-passthrough-with-vfio-on-linux)
- [GPU virtualization with VFIO, NVIDIA AI Enterprise, and AMD SR-IOV -- cloudrift.ai](https://www.cloudrift.ai/blog/gpu-virtualization-qemu-kvm-nvidia-amd)
- [Guide to GPU virtualization: passthrough, vGPU, and MIG -- The Register](https://www.theregister.com/on-prem/2026/04/16/guide-to-gpu-virtualization-passthrough-vgpu-and-mig/5221356)
- [NVIDIA Open GPU Kernel Modules source analysis -- eunomia](https://eunomia.dev/zh/blog/posts/nvidia-open-driver-analysis/)
- [NVIDIA/open-gpu-kernel-modules -- DeepWiki](https://deepwiki.com/NVIDIA/open-gpu-kernel-modules)
