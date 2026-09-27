# sw-os-ml -- MLOS

An operating system, written from scratch in Rust, that treats machine
learning state as its primary virtualized resource.

Conventional operating systems virtualize a scarce resource -- physical
memory -- behind an abstraction the hardware understands: the page.
MLOS virtualizes a different scarce resource -- resident model state --
behind an abstraction the hardware does *not* yet understand: the ML
object. Weights, KV blocks, MoE expert streams, activations, query
context, embeddings and adapters are kernel objects with residency,
tiering, leases and faults, the way pages are in Unix.

This is not a Linux or BSD derivative. It is a new kernel.

Blog post: **[Made Visible: MLOS](https://blog.softwarewrighter.com/2026/09/13/made-visible-mlos/)** -- visualizing this OS.

## Status: four of eight gates, and the thesis has been tested

| Gate | State |
| --- | --- |
| G1 boots to a shell | **done** |
| G2 holds an object table | **done** |
| G3 faults, and the fault carries meaning | **done** |
| G4 known-next-use beats LRU | **done -- this is the one that matters.** 42--77% fewer provider reads than the better of FIFO and LRU on a real 1B model's shape, in the band where any policy can differ; kernel = simulator to the integer. [docs/g4-report.md](docs/g4-report.md) |
| G5-G8 sharing, degradation, host resources, distribution | not started |
| Architectures | **two**: aarch64 (Apple Silicon, native under HVF) and x86-64 (QEMU `microvm`, PVH). The same kernel source, the same shell, the same replay counts to the integer |

The three that are met are all *mechanism*. An object table with tiers
and a fault handler is, to a fair sceptic, a cache with extra steps --
and the sceptic is right so far.

The one thing that makes MLOS not-a-cache is `next_use`: the field saying
when an object will be wanted again, which no page-based system can hold.
It existed for two milestones with nothing to write it. As of M3 a
workload declares its access order to the kernel (`ml_stream_declare`),
the kernel writes the field, and a policy acts on it when the arena is
full -- in the simulator and, since step 009, in the kernel itself, with
the two producing identical counts. Gate G4 is met on that basis:
[docs/g4-report.md](docs/g4-report.md) states the trace, the budget, the
policy rules and the commands, and what it does not show.

[docs/status.md](docs/status.md) is the ground truth -- if it is not in
that file, it does not work. It describes `main`; work in flight on
branches is listed under [Recent, current and planned](#recent-current-and-planned).

## See it run

Two architectures, one kernel. Both recordings are VHS tapes under
`demos/`, served as optimised GIFs from
[sw-ml-study.github.io/sw-os-ml](https://sw-ml-study.github.io/sw-os-ml/)
(`pages/`, deployed by `scripts/deploy-pages.sh`); a GIF of a terminal is
smaller than the same recording as WebP, and `scripts/render-demos.sh`
says by how much. The commands typed are the ones under [Try it](#try-it).

### aarch64 -- Apple Silicon, QEMU/HVF

[![MLOS on aarch64: boot, register the model, a fault then a hit, and the M3 replay under LRU and next-use](https://sw-ml-study.github.io/sw-os-ml/aarch64.gif)](https://sw-ml-study.github.io/sw-os-ml/#aarch64)

A native guest on the M-series cores. `model 32` registers the synthetic
transformer against a 32 KiB arena, a quarter of the model. The first
`get 3 7` faults the tile in off virtio-blk; the second is a hit and
costs nothing. Then `replay lru` and `replay next-use` run the M3
workload **in the kernel** under two eviction policies -- the same policy
crates the simulator runs, producing the same integers -- and next-use
reads less. Recorded by `demos/aarch64.tape`.

### x86-64 -- QEMU `microvm`, TCG

[![MLOS on x86-64: boot via PVH, the machine as found, the model from virtio-blk, a timed sweep](https://sw-ml-study.github.io/sw-os-ml/x86-64.gif)](https://sw-ml-study.github.io/sw-os-ml/#x86-64)

The same source, booted through PVH into 32-bit protected mode and
brought up to long mode by `mlos-hal-x86-64`: COM1, LAPIC and IOAPIC, a
TSC clock, and the identical virtio-blk driver with zero new lines.
`dev` and `mem` show what the PVH loader handed over, `model 32` faults
the model in over virtio-mmio with its provenance nibble, `sweep` is
timed off the TSC. Recorded by `demos/x86-64.tape` on an x86-64 Linux
host; on a Mac the same tape runs under TCG. Details in
[docs/status-x86-64.md](docs/status-x86-64.md).

## What has been measured

Real numbers from real runs, not estimates. Reproduce them with the
commands under [Try it](#try-it).

| | QEMU/HVF (native) | QEMU/TCG |
| --- | --- | --- |
| One model fault, off virtio-blk | ~42 us | ~91 us |
| Recording one residency event | ~4 ns | ~129 ns |
| Sweep of 32 resident tiles, no I/O | ~4.9 us | ~40 us |

Recording costs about one ten-thousandth of a fault, which is why it is
on by default. Method and caveats are in
[docs/status.md](docs/status.md#what-tracing-costs); the first attempt to
measure it found nothing, because sweeps that fault are dominated by 32
virtio round trips.

From a running guest, at an 8 KiB arena against a 144 KiB model:

```
registered 136 objects, 144 KiB across 3 tiers
sweep   acquired 7 of 128 tiles in 364.708 us
        stopped: NoBudget -- no eviction policy yet
Rm      8/144 KiB of the model = 55 per mille
```

That was a faithful picture of the *problem* at M2: memory is a fraction
of the model, the sweep runs out, and MLOS refused rather than guessing
what to throw away. As of M3 step 009 the same shell says `replay
next-use` and the kernel chooses.

**The claim the project rests on has been measured.** On a decode loop
shaped by a real 1.08-billion-parameter Llama, a policy told when each
object is next wanted does **42% to 77% fewer provider reads** than the
better of FIFO and LRU, and moves a third to a tenth of the bytes, in
the band of budgets where any policy can differ. Below the band every
baseline access misses and nothing helps much; above it everything fits
and nothing differs. Where the band sits is a property of the model and
the serving regime -- a small grouped-query model at short context is a
pure weight sweep -- and that is the milestone's second finding. The
kernel is told the future through `ml_stream_declare`, and it produces
the same integers as the simulator on both architectures.
[docs/g4-report.md](docs/g4-report.md) has the tables, the method, and
what it does not show: the traces are derived from a real checkpoint,
not recorded from an inference engine.

## Documents

| Document | What it answers |
| --- | --- |
| [docs/PRD.md](docs/PRD.md) | What MLOS is for, and what "done" means |
| [docs/architecture.md](docs/architecture.md) | Kernel concepts, and the hardware/hypervisor reality we build on |
| [docs/design.md](docs/design.md) | Crates, syscalls, object table, device contracts |
| [docs/plan.md](docs/plan.md) | Milestones and the implementation sagas |
| [docs/status.md](docs/status.md) | Ground truth |
| [docs/layout-handoff.md](docs/layout-handoff.md) | What MLOS emits for the cross-repo visualization, and what it needs back |
| [docs/external-asks.md](docs/external-asks.md) | What MLOS needs from emufpga, sw-mlpl, demo-extensions, sw-tos and demo-memory -- and what it does without each |
| [docs/clustering.md](docs/clustering.md) | How MLOS becomes many instances: transports, homogeneous and heterogeneous clusters, and what each measures |
| [docs/g4-report.md](docs/g4-report.md) | **Gate G4.** Known-next-use against demand, FIFO and LRU on a real model's shape: the tables, the band, PRD Q1 and Q2, how to reproduce it, what it does not show |
| [docs/m3-verdict.md](docs/m3-verdict.md) | The step 006 verdict on the synthetic model, kept as the record of what was measured before the kernel could run any of it |
| docs/research.txt | Raw source material the architecture was distilled from |

## Try it

![Booting MLOS and asking it what it is](docs/tour.gif)

```sh
cargo run -p mlos-cli -- doctor
cargo run -p mlos-cli -- run
```

`doctor` reports what is installed and what is missing. `run` builds the
kernel for the host's architecture -- aarch64 on Apple Silicon, x86-64
on a Linux PC -- and boots it under QEMU with the console on your
terminal; `mlsh` is on the other end. `mlos --arch x86-64 run` picks the
other one explicitly, under TCG where the host cannot accelerate it.
Quit with `Ctrl-A x` (`Ctrl-D` on x86-64); type `help` inside for what it
can tell you.

Inside, `model 8` registers a synthetic transformer against an 8 KiB
arena, `get 3 7` acquires one weight tile, `sweep` walks the whole model
and `arena` says how much of it fits. Running `get` on the same tile
twice is the shortest demonstration of what an object table is for: the
second time costs nothing.

(Do not paste a trailing `# comment` after these: interactive zsh does not
treat `#` as a comment, so it arrives as an argument. `mlos` will now say
so rather than passing it to QEMU.)

## Looking at it

```sh
cargo run -p mlos-cli -- layout
cargo run -p mlos-cli -- runtime
```

`layout` describes what the build produced; `runtime` boots MLOS, sweeps
the model and describes what the running system holds. Both write the
columnar `sw-ml-study.system-layout` contract that `sw-tos`
also emits and `sw-mlpl` renders: where every weight tile sits on disk,
how the kernel image divides RAM, what is resident in the arena, and --
from `runtime` -- a stream of residency events saying how it got that way.

Conforming samples are committed under `examples/viz/` so the sibling
repositories can develop against them without running MLOS. The contract,
the vocabulary and the open questions are in
[docs/layout-handoff.md](docs/layout-handoff.md).

## How this uses Apple Silicon

As a **host**, and only as a host. MLOS boots on two architectures, and
on an M-series Mac the aarch64 build runs as a native guest: QEMU with the `hvf` accelerator
hands the guest's ARM instructions to Hypervisor.framework, which runs
them on the real cores. Nothing translates an instruction set.

The evidence is in the boot banner. Under HVF the guest reads
`CNTFRQ_EL0` and gets **24 MHz** -- Apple's own counter frequency, passed
through. Under TCG the same read gets 62.5 MHz, which is QEMU's virtual
timer. MLOS reads the register rather than assuming a rate, which is why
the same binary times itself correctly on both.

What MLOS *drives* is QEMU's virtual hardware, not Apple's: a GICv3
interrupt controller (Apple Silicon has an AIC, which MLOS never sees), a
PL011 or virtio console, and virtio-mmio block devices. That is the
point -- the kernel is portable to any aarch64 machine QEMU can present,
and the x86-64 build boots under QEMU `microvm` -- on this Mac under
TCG, on a Linux PC under KVM once saga `mlos-two-hosts` lands.

**The Apple GPU is not used at all.** No Metal, no ANE, nothing. It is
reached at M6, and not by MLOS driving it: the host does, behind a narrow
`virtio-ml-compute` interface, while MLOS decides what should be resident
in device memory and when. MLOS is a control plane for ML state, not a
GPU operating system, and a from-scratch GPU driver would spend the
effort reproducing what the host already does well.
[docs/architecture.md](docs/architecture.md) s.7.4 draws the boundary.

Virtualization.framework (via `vfkit`) boots the same image and produces
no console output yet: it offers a virtio console and no PL011, and the
virtio-pci transport that would fix it arrives with M6.

## Recent, current and planned

Development is **parallel**: each saga runs on its own branch (a
`feat/<slug>` branch becomes `pr/<slug>` when it is ready for review,
per [AGENTS.md](AGENTS.md)), `main` is the integration point, and
[docs/status.md](docs/status.md) describes `main` only. More than one
agent may be working at once; the plan is what keeps them from colliding.

**Recent:**

- **M3 is complete and gate G4 is met** (saga
  [`mlos-nextuse`](docs/plan.md#saga-mlos-nextuse-m3), eleven steps).
  The simulated verdict; `ml_stream_declare` so the kernel writes
  `next_use`; a real allocator that can evict; the same policy crates in
  the kernel matching the simulator to the integer (PR #2); a trace
  shaped by a real 1B-parameter checkpoint through emufpga's importer
  (PR #4); and the report, [docs/g4-report.md](docs/g4-report.md).
- Saga [`mlos-x86-64`](docs/plan.md#saga-mlos-x86-64-portability-no-gate),
  nine steps, done and merged as PR #3: PVH boot, a 16550 console, LAPIC
  and IOAPIC, a TSC clock, virtio-blk with no new driver code, and the
  M3 replay producing the same four count lines on x86-64 as on aarch64
  and in the simulator. It ran in its own lane (`lanes/x86/`) alongside
  M3.

**Current:** nothing in flight. The next two sagas can run in parallel.

**Planned**, in order, from [docs/plan.md](docs/plan.md):

- Saga [`mlos-two-hosts`](docs/plan.md#saga-mlos-two-hosts-portability-no-gate):
  a Linux x86-64 machine beside the Mac, each booting both guests (HVF
  and TCG on the Mac; KVM and TCG on Linux) and running the whole gate.
  Restores what the deleted CI runner uniquely offered: a machine that is
  not this one.
- M4 `mlos-parameter-major` (gate G5): one provider read serves N
  sessions. It inherits two findings from G4: state budgets as a fraction
  of the per-token working set and report where the band is, and page
  the KV cache rather than one block per token.
- M5 `mlos-degradation` (G6): contracts, admission, the degradation
  ladder. M6 onward: host resources over narrow virtio interfaces, then
  distribution, heterogeneity, global scheduling, and the ML-MMU last.

## Development

MLOS boots in a virtual machine, never on bare metal. Apple Silicon is
the primary development host; a Linux/NVIDIA host is the second target,
where real GPU passthrough becomes possible.

Work is tracked with [agentrail](https://softwarewrighter.com) sagas.
`agentrail next` prints the current step. Sagas run in parallel on
separate branches or git worktrees; see
[Recent, current and planned](#recent-current-and-planned) for what is
in flight and [docs/plan.md](docs/plan.md) for what comes next.

---

Copyright (c) Software Wrighter LLC.
