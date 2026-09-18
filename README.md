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

## Status: three of eight gates, and the thesis is untested

| Gate | State |
| --- | --- |
| G1 boots to a shell | **done** |
| G2 holds an object table | **done** |
| G3 faults, and the fault carries meaning | **done** |
| G4 known-next-use beats LRU | **not started -- this is the one that matters** |
| G5-G8 sharing, degradation, host resources, distribution | not started |

The three that are met are all *mechanism*. An object table with tiers
and a fault handler is, to a fair sceptic, a cache with extra steps --
and the sceptic is right so far.

The one thing that makes MLOS not-a-cache is `next_use`: the field saying
when an object will be wanted again, which no page-based system can hold.
**Nothing writes it.** It has existed since the object table was built,
every layout document emits it as `never`, and until a policy acts on it
this project has demonstrated a *problem* rather than a solution.

That is deliberate sequencing, not an oversight, and
[docs/status.md](docs/status.md) is the ground truth -- if it is not in
that file, it does not work.

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

That is a faithful picture of the *problem*: memory is a fraction of the
model, the sweep runs out, and MLOS refuses rather than guessing what to
throw away. It is not yet a picture of a solution.

**What has not been measured is the claim the project rests on:** that an
operating system which is told the future beats one that guesses. The
next four steps produce that number -- see
[Where this goes next](#where-this-goes-next).

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
| docs/research.txt | Raw source material the architecture was distilled from |

## Try it

![Booting MLOS and asking it what it is](docs/tour.gif)

```sh
cargo run -p mlos-cli -- doctor
cargo run -p mlos-cli -- run
```

`doctor` reports what is installed and what is missing. `run` builds the
kernel, turns it into a bootable arm64 image and boots it under QEMU with
the console on your terminal -- `mlsh` is on the other end. Quit with
`Ctrl-A x`; type `help` inside for what it can tell you.

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

As a **host**, and only as a host. MLOS is an aarch64 kernel, so on an
M-series Mac it runs as a native guest: QEMU with the `hvf` accelerator
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
and a second x86-64 target is already built in CI.

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

## Where this goes next

The next milestone is the one that justifies the project: replay one
workload against four residency policies under an identical memory
budget, and see whether knowing the future wins.

Four host-side steps produce that table -- a replay harness, three
baselines, a workload with real reuse in it, and the known-next-use
policy. None of them needs an emulator or anything from another
repository. Then the saga stops and looks at the answer, because
[the plan](docs/plan.md#saga-mlos-nextuse-m3) commits to the other
outcome: if known-next-use does not separate from LRU on a fair
workload, say so and stop before M4.

## Development

MLOS boots in a virtual machine, never on bare metal. Apple Silicon is
the primary development host; a Linux/NVIDIA host is the second target,
where real GPU passthrough becomes possible.

Work is tracked with [agentrail](https://softwarewrighter.com) sagas.
`agentrail next` prints the current step.

---

Copyright (c) Software Wrighter LLC.
