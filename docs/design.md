# MLOS -- Design

Status: draft, pre-implementation. Nothing here is built yet; see
[status.md](status.md).
Read [architecture.md](architecture.md) first -- this document is the
"how", and assumes the "what" and "why".

---

## 1. Language and toolchain

**Rust 2024 edition**, everywhere, no exceptions. `edition = "2024"` in
every `Cargo.toml`; `sw-checklist` fails the build otherwise.

```toml
[workspace.package]
edition      = "2024"
rust-version = "1.85"          # 2024 edition floor; CI pins the exact toolchain
license      = "MIT OR Apache-2.0"
```

What we take from the 2024 edition specifically:

- **`unsafe_op_in_unsafe_fn` is deny-by-default.** An `unsafe fn` body
  no longer gets an implicit unsafe block. In a kernel where every MMIO
  access is unsafe, this forces each hazardous operation to be named
  and justified individually rather than hiding inside a function
  signature.
- **`unsafe` required on `extern` blocks and on dangerous attributes**
  (`#[no_mangle]`, `#[link_section]`). Kernel entry points and linker
  section placement become visible as unsafe rather than incidental.
- **RPIT lifetime capture and the new match ergonomics** clean up the
  provider trait signatures, which are otherwise lifetime-noisy.
- **`gen` blocks** are the natural way to express a `STREAM`'s
  descriptor sequence.

Targets:

| Target | Purpose |
| --- | --- |
| `aarch64-unknown-none-softfloat` | Kernel on Apple Silicon (QEMU `virt`) and arm64 Linux hosts |
| `x86_64-unknown-none` | Kernel on the Linux/NVIDIA host |
| host triple | Simulators, trace tools, the `mlos` CLI, all tests that are not on-target |

Softfloat on aarch64 because the kernel must not touch FP/SIMD registers
without saving them, and the kernel does no math worth the risk. Tensor
math is userspace's job (architecture s.1, commitment 1).

Gates before every `agentrail complete`:

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace                     # host-side
cargo build --target aarch64-unknown-none-softfloat -p mlos-kernel
cargo build --target x86_64-unknown-none   -p mlos-kernel
sw-checklist                               # zero failures, zero warnings
```

## 2. Crate layout

`sw-checklist` caps crates at 7 modules (fails) and warns above 4, so
the crate graph is deliberately wide and shallow. Design to <=4 modules
per crate; a crate at 4 that needs a fifth concern gets a *sibling
crate*, not a fifth module.

```
sw-os-ml/
  crates/
    mlos-abi/         syscall numbers, ML object ids, error codes, wire structs
                      no_std, no alloc, forbid(unsafe_code)   [shared kernel<->user]
    mlos-hal/          trait Hal: mmu, timer, irq, cpu, console       [unsafe allowed]
    mlos-hal-aarch64/  aarch64 impl: EL1 boot, GICv3, generic timer, TTBR
    mlos-hal-x86-64/   x86-64 impl: long-mode boot, APIC, TSC, CR3
    mlos-kernel/       entry, scheduler, ipc, capabilities, fault dispatch
    mlos-objtab/       the ML object table: ids, metadata, residency state
    mlos-provider/     trait Provider + the resolution path
    mlos-provider-dram/
    mlos-provider-block/
    mlos-provider-virtio/   virtio-blk, virtio-fs, virtio-console
    mlos-provider-device/   GPU VRAM across PCIe (BAR mapping, DMA)
    mlos-provider-mlmmu/    the FPGA card, real or emulated
    mlos-policy/       trait Policy + demand/fifo/lru                  [no_std]
    mlos-policy-nextuse/    known-next-use (Belady)
    mlos-policy-cost/       reload vs recompute vs quantize
    mlos-policy-router/     MoE probability prefetch
    mlos-metrics/      the Rm/Ps/Bt/Rc/Pf/Ph/Ks/Ss counters
    mlos-pci/          PCIe enumeration, BAR mapping, MSI-X
    mlos-sim/          host-side simulator: replay traces, run policies   [std]
    mlos-trace/        trace format, record/replay                        [std]
    mlos-cli/          `mlos` -- build images, run VMs, replay traces      [std]
  scripts/             fetch/build firmware, launch VMs, no artifacts committed
  docs/
```

Two structural rules that keep this honest:

1. **`mlos-policy*` crates are `no_std` and pure.** A policy takes the
   object table's view and returns a decision. It performs no I/O and
   holds no state the table does not own. That is what lets the *same
   code* run in the kernel and in `mlos-sim` against a recorded trace.
2. **`mlos-abi` is the only crate both sides depend on.** If a type
   crosses the syscall boundary it lives there, and it is
   `#[repr(C)]`, `Copy`, and has a stable layout test.

`unsafe` is permitted only in `mlos-hal*`, `mlos-pci`,
`mlos-provider-{virtio,device,mlmmu}`. Every other crate carries
`#![forbid(unsafe_code)]`, enforced by a test that greps the tree.

## 3. Boot

### 3.1 aarch64 under QEMU `virt` + HVF

```
  QEMU -M virt -cpu host -accel hvf
        |
   EDK2 AAVMF (UEFI)        or   -kernel mlos.bin (direct, dev loop)
        |
   mlos-hal-aarch64::_start      EL1, MMU off, x0 = DTB pointer
        |
   parse DTB -> memory map, GIC base, timer freq, virtio mmio nodes
        |
   set up TTBR0/TTBR1, enable MMU, switch to a real stack
        |
   mlos_kernel::main()
```

Two boot paths on purpose. `-kernel` direct boot is the fast
development loop (no image build, no firmware). UEFI is how the same
binary boots under Virtualization.framework, which only offers
`VZEFIBootLoader` -- so the UEFI path is what keeps requirement N2
(two hypervisors per arch) satisfiable.

The Virtualization.framework constraint from architecture s.8.1 is a
build-system requirement, not a runtime one: **the image must be an
uncompressed raw arm64 image.** A compressed EFI zboot kernel fails
with an opaque internal error because no firmware is present to
decompress it. `mlos-cli` emits uncompressed by default and refuses to
compress for that target.

### 3.2 x86-64 under QEMU `q35` + KVM

```
  QEMU -M q35 -accel kvm
        |
   OVMF (UEFI)
        |
   UEFI stub -> mlos-hal-x86-64::_start   long mode, boot info structure
        |
   ACPI: MADT (APIC), MCFG (PCIe ECAM), HPET
        |
   page tables, GDT, IDT
        |
   mlos_kernel::main()
```

We do not support legacy BIOS or multiboot. UEFI on both architectures
means one image-format story.

### 3.3 What the HAL trait must cover, and nothing more

```rust
pub trait Hal {
    type PageTable: PageTable;
    fn boot_info(&self) -> &BootInfo;         // memory map, device tree / ACPI
    fn timer(&self) -> &dyn Timer;
    fn irq(&self) -> &dyn IrqController;
    fn console(&self) -> &dyn Console;
}
```

Four methods, because `sw-checklist` warns above four and because a
wider HAL is a HAL that has started leaking architecture into the
kernel. Nothing above `mlos-hal` may name a page size, a privilege
level, an interrupt controller, or an atomics width -- that is the
concession that keeps `mlos-hal-riscv64` an additive change
(architecture s.9).

## 4. The ML object table

### 4.1 Object identity

```rust
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectId(u64);
```

Structured, not opaque, so the ML-MMU can decode it in gateware without
a lookup:

```
 63    56 55        40 39        24 23        8 7      0
+--------+------------+------------+-----------+--------+
| class  |   model    |   layer    |  tensor   |  tile  |
+--------+------------+------------+-----------+--------+
   8 bits    16 bits      16 bits     16 bits    8 bits
```

- `class` -- `WEIGHT_TILE`, `SCALE`, `EXPERT`, `KV_BLOCK`,
  `ACTIVATION`, `EMBED_BLOCK`, `RAG_BLOCK`, `ADAPTER`.
- Session-scoped classes (`KV_BLOCK`, `ACTIVATION`) reuse the `model`
  field as a session id. The class disambiguates.
- 256 tiles per tensor is the granularity ceiling. PRD Q1 asks whether
  tiles are the right unit at all; the field costs nothing to carry and
  is set to 0 for tensor-granular objects until G4 answers it.

The layout is fixed in `mlos-abi` with a compile-time layout test,
because the ML-MMU register contract (s.8) hard-codes these bit
positions in gateware.

### 4.2 Metadata

```rust
#[repr(C)]
pub struct ObjectMeta {
    pub size:           u32,
    pub precision:      Precision,     // FP16 | BF16 | Q8 | Q4 | Q3 | TERNARY
    pub tier:           Tier,          // HOT | WARM | COLD | STREAM | ARCHIVE
    pub provider:       ProviderId,
    pub next_use:       NextUse,       // Never | Distance(u32) | Probability(u16)
    pub reuse_count:    u16,
    pub reload_cost:    CostNs,
    pub recompute_cost: CostNs,        // CostNs::IMPOSSIBLE for weights
    pub share_count:    u16,           // live leases; drives parameter-major
    pub mutability:     Mutability,    // Immutable | CowOverlay | Mutable
    pub owner:          SessionId,
}
```

`NextUse` is a three-way enum rather than a number because the two
stream kinds differ in kind, not degree: a dense sweep yields an exact
`Distance`, an MoE router yields a `Probability`. An eviction policy
may act on `Distance` with certainty and on `Probability` only as a
hint. Collapsing them to one number would silently license the wrong
decision.

### 4.3 Storage

A flat open-addressed hash table over `ObjectId`, sized at boot from
the residency budget, in a kernel arena. Not a tree: the fast path is
an exact-match lookup and must be a couple of cache lines, not a
traversal.

The table is in the kernel (architecture s.4, PRD Q2). *Policy* is not:
the table calls out to a policy service on eviction and prefetch
decisions only, never on the resident fast path.

## 5. Syscall surface

Small on purpose. Every syscall goes through `mlos-abi`.

### 5.1 Objects

```
ml_register(id, meta, provider)     -> Result<()>
ml_resolve(id)                      -> Result<Placement>
ml_acquire(id, lease_kind, deadline)-> Result<Handle>     // may MODEL_FAULT
ml_release(handle)                  -> Result<()>
ml_prefetch(id, hint)               -> Result<()>         // SPECULATIVE lease
ml_evict_hint(id, verb)             -> Result<()>         // userspace advice
```

### 5.2 Streams

```
ml_stream_declare(kind, sequence)   -> Result<StreamId>
ml_stream_advance(stream)           -> Result<ObjectId>
ml_stream_route(stream, dist)       -> Result<()>         // MoE: update probabilities
ml_stream_end(stream)               -> Result<()>
```

`ml_stream_declare` is how a userspace runtime tells the kernel the
future it already knows. Everything in architecture s.5 that makes
`next_use` policy possible depends on this one call being used.

### 5.3 Sessions

```
ml_session_create(contract)         -> Result<SessionId>  // may be REFUSED
ml_session_budget(session, bytes)   -> Result<()>
ml_session_contract(session)        -> Result<Contract>   // what was delivered
ml_session_destroy(session)         -> Result<()>
```

`Contract { quality_floor, latency_ceiling, resident_ceiling }`.
`ml_session_create` refusing is admission control (architecture s.5,
degradation rung L6) -- it is a normal outcome, not an error condition.

### 5.4 Capabilities, IPC, metrics

```
cap_derive(cap, rights)             -> Result<Cap>        // attenuate only
cap_send(endpoint, cap)             -> Result<()>

ipc_send(endpoint, msg)             -> Result<()>
ipc_recv(endpoint)                  -> Result<Message>

metrics_read(class)                 -> Result<Counters>
metrics_snapshot()                  -> Result<TraceFrame>
```

**Tensor-handle IPC (PRD F6).** A message body may carry a
`Handle` + `Lease`, transferring residency ownership without copying
bytes. Camera -> encoder -> classifier moves handles, not tensors.
`cap_derive` can only attenuate rights, never widen them; there is no
ambient authority over a GPU or a model.

That is 20 calls. The temptation to grow this is the main way a
microkernel stops being one.

## 6. Provider interface

```rust
pub trait Provider {
    fn resolve(&self, id: ObjectId) -> Result<Placement>;
    fn read(&self, id: ObjectId, off: u32, into: &mut [u8]) -> Result<u32>;
    fn prefetch(&self, id: ObjectId, hint: Hint) -> Result<()>;
    fn cost(&self, id: ObjectId) -> Cost;
}
```

Four methods (the `sw-checklist` gate again, and it happens to be the
right number). `cost()` is not optional: an eviction policy that cannot
ask what recovery would cost is guessing, and guessing is what LRU
does.

| Provider | Backing | Notes |
| --- | --- | --- |
| `dram` | kernel arena | `cost` ~ 0; the destination tier, not really a source |
| `block` | virtio-blk | COLD tier |
| `hostfile` | virtio-fs | dev convenience: model files on the host, no image rebuild |
| `device` | GPU VRAM over PCIe | see s.7 |
| `recompute` | calls back into userspace | the only provider that runs guest code to satisfy a fault |
| `mlmmu` | the FPGA card | see s.8 |

`recompute` is architecturally odd and deliberately so: it is the
provider that makes `DISCARD` a legitimate eviction verb. It resolves a
fault by scheduling the userspace producer that can rebuild the object,
rather than by reading bytes.

## 7. GPU: what gets written, in what order

Restating architecture s.7 as work items, so the scope stays bounded:

### 7.1 Tier A -- managed placement (PoC gate G7, Linux/NVIDIA)

`mlos-pci` + `mlos-provider-device`:

1. Enumerate PCIe via ECAM (from ACPI MCFG on x86-64, DTB on aarch64).
2. Identify the passed-through GPU by class code and vendor id.
3. Map BARs. **Require resizable BAR** so all of VRAM is addressable
   rather than through a 256 MB window -- an object manager doing
   placement across PCIe needs the whole aperture (architecture s.7.3).
4. Set up a DMA path and move a tensor into device memory under
   `ml_acquire` control, accounted as tier `HOT`.

No command submission, no shader compilation, no GSP RPC. G7 is
*placement*, and stopping there is what stops this from becoming a
driver project (risk table, architecture s.12).

### 7.2 Tier B -- paravirtual compute (Apple Silicon, and Linux)

`mlos-provider-device` grows a virtio-gpu front end speaking the Venus
protocol. The host side is libkrun's `virglrenderer` -> MoltenVK ->
Metal (architecture s.7.1). This is the path by which MLOS actually
runs a model on the Mac, and it is a bounded implementation against a
documented protocol.

Sequencing note: Tier B needs virtio working well anyway
(`mlos-provider-virtio`), so the virtio work is shared with the block
and console paths and is not GPU-specific cost.

### 7.3 Tier C -- native

Not planned. Recorded in architecture s.7.2 because the GSP firmware
architecture makes it conceivable, not because we intend it.

## 8. The ML-MMU register contract

The artifact that lets emufpga build gateware against software that
already exists. Gen 0 is a QEMU device model implementing exactly this
(PoC gate G8); Gen 1+ is real hardware implementing exactly this.

### 8.1 BAR0 -- control and translation

| Offset | Reg | Access | Meaning |
| --- | --- | --- | --- |
| `0x000` | `ID` | RO | magic + gen number |
| `0x008` | `CAPS` | RO | bitmask: DEQUANT, GATHER, ROUTER, NEXTUSE, CRC |
| `0x010` | `CTRL` | RW | enable, reset, IRQ mask |
| `0x018` | `STATUS` | RO | busy, error, ring state |
| `0x020` | `TT_BASE` | RW | translation table base (guest physical) |
| `0x028` | `TT_SIZE` | RW | entries |
| `0x030` | `RING_BASE` | RW | descriptor ring base |
| `0x038` | `RING_SIZE` | RW | entries |
| `0x040` | `RING_HEAD` | RW | producer index (software writes) |
| `0x048` | `RING_TAIL` | RO | consumer index (device writes) |
| `0x050` | `FAULT_ID` | RO | `ObjectId` of the last untranslatable access |
| `0x058` | `FAULT_INFO` | RO | reason, offset |

`CAPS` is how one kernel binary drives every ML-MMU generation: the
`mlmmu` provider advertises only the functions the card reports, and
falls back to software for the rest. Gen 0 emulated reports all caps;
Gen 1 gateware reports `DEQUANT` off, and nothing above the provider
notices.

### 8.2 Translation table entry (16 bytes)

```
  +0   ObjectId          u64   (the s.4.1 bit layout, decoded in gateware)
  +8   address           u48   guest-physical, or provider-relative
  +14  tier              u4    HOT | WARM | COLD | STREAM | ARCHIVE
  +14  encoding          u4    FP16 | BF16 | Q8 | Q4 | Q3 | TERNARY
  +15  flags             u8    valid, dirty, pinned, streaming
```

Deliberately 16 bytes: four entries per cache line, and a power of two
so the gateware indexer is a shift.

### 8.3 Descriptor ring

One descriptor kind, with an opcode, because a stream engine that needs
five descriptor formats is a stream engine nobody will build:

```
  op        FETCH | PREFETCH | DEQUANT | GATHER | ROUTE | FLUSH
  object    ObjectId
  offset    u32
  length    u32
  dest      u64        guest-physical destination
  aux       u64        op-specific: gather list ptr, router dist ptr
  flags     u32        interrupt-on-completion, double-buffer slot
```

`ROUTE` is the interesting one: hand the device a router probability
distribution and it issues its own `PREFETCH`es down the datapath,
which is architecture s.10.1's "expert routing in gateware."

### 8.4 Next-use counters

When `CAPS.NEXTUSE` is set, the device maintains the `next_use` and
`reuse_count` fields in the translation table as a side effect of
servicing descriptors. That is the whole point of hardware here: the
eviction policy reads the numbers it needs without a fault and without
the CPU touching the stream.

## 9. Testing without hardware

The rule (PRD N3): **a policy that cannot be replayed cannot be
committed.**

```
   real model (via emufpga importers)
        |
        v
   mlos-trace: record layer/expert/KV access order
        |
        +---> mlos-sim (host, std, no VM) ----> policy comparison table
        |         same mlos-policy-* crates the kernel links
        |
        +---> QEMU TCG (no accel, deterministic) ----> kernel integration
        |
        +---> QEMU HVF / KVM ----> the real thing
```

Four levels, each catching what the level below cannot:

1. **Unit tests, host.** `mlos-objtab`, `mlos-policy*`, `mlos-abi`
   layout. Pure, fast, no VM.
2. **Simulation, host.** `mlos-sim` replays a recorded trace against
   every policy under an identical budget and emits the G4/G5
   comparison tables. This is where policy research happens, and it is
   where the demo-memory work plugs in.
3. **TCG integration.** Same kernel binary, no acceleration, fully
   deterministic. This is CI. A test that fails here fails the same way
   every time, which is what makes kernel bugs findable.
4. **Accelerated.** HVF or KVM. Real timing, real GPU, real answers.
   Not deterministic, so it validates rather than gates.

Trace fidelity is a stated risk (architecture s.12): traces come from
real models through emufpga's importers, never hand-written, or the
G4/G5 results are constructed and worthless.

## 10. `mlos` CLI

One binary, because `sw-checklist` validates `--help`/`--version` on
CLI binaries and one well-formed CLI is cheaper than six.

```
mlos build   --target aarch64|x86-64 [--uefi|--raw]
mlos run     --host hvf|kvm|tcg|vz|krun [--gpu <pci-addr>]
mlos trace   record|replay
mlos sim     --trace <t> --policy <p> --budget <bytes>
mlos doctor                        # what is installed, what is missing
```

`mlos doctor` earns its place: the host requirements differ sharply
between the Mac and the Linux box (QEMU with HVF, EDK2 firmware,
libkrun, VFIO binding, IOMMU groups, hugepages), and "why does this not
boot" should be answerable by a command rather than by rereading
architecture s.8.

## 11. Conformance

Standing rules, restated here because they are design constraints
rather than style preferences:

- <=4 functions per module, <=4 modules per crate, <=25 LOC per
  function. Design to the *gate*, never to the fail line -- a module
  left at exactly 7 functions is a failure waiting for the next
  one-line change.
- `#![forbid(unsafe_code)]` in every crate outside `mlos-hal*`,
  `mlos-pci`, and the three device-touching providers. Enforced by a
  test, not by convention.
- Every `unsafe` block carries a `// SAFETY:` comment naming the
  invariant relied on. The 2024 edition's
  `unsafe_op_in_unsafe_fn` makes this enforceable inside `unsafe fn`
  bodies too.
- Zero `sw-checklist` failures and zero warnings, from the first commit
  onward. Commits that hold or grow the count carry
  `sw-checklist: exception` with a justification.
