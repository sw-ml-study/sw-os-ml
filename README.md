# sw-os-ml -- MLOS

An operating system, written from scratch in Rust, that treats
machine-learning state as its primary virtualized resource. It is a
guest, not a host: it decides where weights, cache blocks, experts and
activations live, and lets the host keep everything else.

**The premise.** Every operating system since Unix manages memory by
reacting. A page is touched, it is missing, something else is thrown out
on a guess about the past. Inference does not need to guess. A
transformer reads its weights in the same order every token and grows
its cache one block at a time, so its future is known before it happens.
MLOS lets a workload hand over that future through a system call, and
replaces the guess with a plan. Measured against LRU on a real model's
shape, the plan reads 42 to 77 percent less where memory is tight.[^band]

**The approach.** MLOS runs in a virtual machine and owns one thing:
which ML objects are resident where, at what precision, at what cost.
Weights, KV blocks, experts and activations are kernel objects with
residency, tiers, leases and faults, the way pages are in Unix. The host
keeps its drivers, filesystem, network and GPU runtime; MLOS reaches them
through narrow virtio interfaces and reimplements none of them. It boots
on aarch64 and x86-64 under QEMU from one source tree, and is not meant
for bare metal.

**What it is not.** Not a Linux or BSD fork. Not a GPU driver. Not an
inference engine. Not, yet, a system anyone runs a model on.

Blog post: **[Made Visible: MLOS](https://blog.softwarewrighter.com/2026/09/13/made-visible-mlos/)**.

## In one screen

| | | Detail |
| --- | --- | --- |
| **Mission** | Find out, by measurement, whether an OS that is told the future of ML state beats one that guesses. Stop if it does not. | [dream.md](docs/dream.md#mission) |
| **Vision** | A distributed control plane for ML state across GPU memory, RAM, SSD and the network, deciding placement the host cannot see. | [dream.md](docs/dream.md#vision) |
| **Requirements** | A `no_std` kernel, VM only, two hypervisors per architecture, `unsafe` confined, every policy replayable on a host before it enters the kernel. | [PRD.md](docs/PRD.md#6-requirements) |
| **Objectives** | Eight gates. **Five met**: it boots, holds objects, faults with meaning, beats LRU with knowledge, serves N sessions with one read. | [status.md](docs/status.md) |
| **Plan** | Milestones M0 to M10, one saga each, in parallel lanes. M0 to M4 done; M5 and `mlos-two-hosts` next. | [plan.md](docs/plan.md) |
| **Built how** | Forty-odd small crates under a size gate; the simulator links the kernel's own policy and arena crates and the kernel must match it to the integer. | [anatomy.md](docs/anatomy.md#how-it-is-built) |
| **Does** | Boots to a shell on two architectures, faults a model in from virtio-blk, is told its access order, evicts by policy, replays a workload and reports the cost. | [anatomy.md](docs/anatomy.md#what-it-does) |
| **How** | A fault names the layer, not an address; a declared stream writes `next_use`; a policy names one victim at a time. | [anatomy.md](docs/anatomy.md#how-it-does-it) |
| **Does not** | Run a model, host processes, own a filesystem or a GPU, run on bare metal. Two lists: not ever, and not yet. | [anatomy.md](docs/anatomy.md#what-it-does-not-do) |

[docs/status.md](docs/status.md) is the ground truth: if it is not in
that file, it does not work. It describes `main`; anything in flight is
on a `pr/*` branch.

## See it run

Two architectures, one kernel. Recordings are VHS tapes under `demos/`,
served as GIFs from
[sw-ml-study.github.io/sw-os-ml](https://sw-ml-study.github.io/sw-os-ml/).

### aarch64 -- Apple Silicon, QEMU/HVF

[![MLOS on aarch64: boot, register the model, a fault then a hit, and the M3 replay under LRU and next-use](https://sw-ml-study.github.io/sw-os-ml/aarch64.gif)](https://sw-ml-study.github.io/sw-os-ml/#aarch64)

`model 32` registers the synthetic transformer against an arena a
quarter its size. The first `get 3 7` faults the tile in off virtio-blk;
the second is a hit. `replay lru` and `replay next-use` run the M3
workload **in the kernel** under two policies, and next-use reads less.

### x86-64 -- QEMU `microvm`, TCG

[![MLOS on x86-64: boot via PVH, the machine as found, the model from virtio-blk, a timed sweep](https://sw-ml-study.github.io/sw-os-ml/x86-64.gif)](https://sw-ml-study.github.io/sw-os-ml/#x86-64)

The same source, booted through PVH into long mode: COM1, LAPIC and
IOAPIC, a TSC clock, and the identical virtio-blk driver with zero new
lines. Details in [docs/status-x86-64.md](docs/status-x86-64.md).

## Try it

![Booting MLOS and asking it what it is](docs/tour.gif)

```sh
cargo run -p mlos-cli -- doctor
cargo run -p mlos-cli -- run
```

`doctor` reports what is installed and what is missing. `run` builds the
kernel for the host's architecture and boots it under QEMU with the
console on your terminal; `mlsh` is on the other end. `mlos --arch
x86-64 run` picks the other architecture, under TCG where the host cannot
accelerate it. Quit with `Ctrl-A x` (`Ctrl-D` on x86-64); type `help`
inside for what it can tell you.

Inside, `model 32` registers a synthetic transformer, `get 3 7` acquires
one weight tile, `sweep` walks the whole model, `arena` says how much of
it fits, and `replay next-use` runs the M3 comparison. Running `get` on
the same tile twice is the shortest demonstration of what an object table
is for: the second time costs nothing.

(Do not paste a trailing `# comment` after these: interactive zsh does
not treat `#` as a comment, so it arrives as an argument.)

## Looking at it

```sh
cargo run -p mlos-cli -- layout
cargo run -p mlos-cli -- runtime
```

`layout` describes what the build produced; `runtime` boots MLOS, sweeps
the model and describes what the running system holds, in the columnar
contract `sw-tos` also emits and `sw-mlpl` renders. Conforming samples
are under `examples/viz/`; the contract is
[docs/layout-handoff.md](docs/layout-handoff.md).

## Measured

Every number is from a run, reproducible with the commands in the
document that states it. A model fault off virtio-blk costs about 42 us
under HVF; recording a residency event about 4 ns
([docs/status.md](docs/status.md#what-tracing-costs)). On a decode loop
shaped by a real 1.08-billion-parameter Llama, known-next-use does 42 to
77 percent fewer provider reads than the better of FIFO and LRU in the
band of budgets where any policy can differ, and the kernel reproduces
the simulator to the integer on both architectures
([docs/g4-report.md](docs/g4-report.md)).

## Documents

| Document | What it answers |
| --- | --- |
| [docs/dream.md](docs/dream.md) | Why: mission, vision, requirements, objectives, plan, one paragraph each |
| [docs/anatomy.md](docs/anatomy.md) | What: how it is built, what it does, how, and what it does not do |
| [docs/PRD.md](docs/PRD.md) | What MLOS is for, and what "done" means |
| [docs/architecture.md](docs/architecture.md) | Kernel concepts, and the hardware/hypervisor reality we build on |
| [docs/design.md](docs/design.md) | Crates, syscalls, object table, device contracts |
| [docs/plan.md](docs/plan.md) | Milestones and the implementation sagas |
| [docs/status.md](docs/status.md) | Ground truth |
| [docs/g5-report.md](docs/g5-report.md) | Gate G5: four sessions, one read; the scheduler, the cache bound, the latency escape's price, the kernel on both architectures |
| [docs/g4-report.md](docs/g4-report.md) | Gate G4: the tables, the band, PRD Q1 and Q2, how to reproduce it, what it does not show |
| [docs/m3-verdict.md](docs/m3-verdict.md) | The step 006 verdict on the synthetic model, kept as a record |
| [docs/status-x86-64.md](docs/status-x86-64.md) | What the x86-64 guest does |
| [docs/layout-handoff.md](docs/layout-handoff.md) | What MLOS emits for the cross-repo visualization |
| [docs/external-asks.md](docs/external-asks.md) | What MLOS needs from its sibling repositories |
| [docs/clustering.md](docs/clustering.md) | How MLOS becomes many instances |
| docs/research.txt | Raw source material the architecture was distilled from |

## Roadmap

Development is parallel: each saga runs on its own branch, `feat/<slug>`
becoming `pr/<slug>` for review ([AGENTS.md](AGENTS.md)), and `main` is
the integration point.

- **Done.** M0 to M4 (five gates), and saga `mlos-x86-64`: the same
  kernel on a second architecture, same replay counts. M4 added sessions
  as kernel objects, live `share_count`, a `no_std` scheduler the kernel
  and the simulator share, the latency escape, and `Ps`, `Ks`, `Ss`.
- **Next, in parallel.** M5 `mlos-degradation` (gate G6): contracts,
  admission control, the degradation ladder. Saga `mlos-two-hosts`: a
  Linux machine beside the Mac, both guests on both, the whole gate on
  both.
- **Then.** M6 host resources over virtio (G7), M7 another machine as a
  provider (G8), heterogeneity, global scheduling, and the ML-MMU last.
  [docs/plan.md](docs/plan.md).

## Development

MLOS boots in a virtual machine, never on bare metal. Work is tracked
with [agentrail](https://softwarewrighter.com) sagas; `agentrail next`
prints the current step. The pre-commit gate, the size rules and the
step protocol are in [AGENTS.md](AGENTS.md).

[^band]: The advantage is a band of memory budgets near the model's
    per-token working set. Below it every reactive policy misses every
    access and knowing the future saves only what fits; above it
    everything fits and no policy differs. Where the band sits depends
    on the model and the serving regime, and the traces were derived
    from a real checkpoint's tensor inventory rather than recorded from
    an inference engine. [docs/g4-report.md](docs/g4-report.md) states
    all of this.

---

Copyright (c) Software Wrighter LLC.
