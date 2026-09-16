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

## Status

Three of eight proof-of-concept gates met. MLOS boots to a shell as a
native aarch64 guest, holds an object table across three tiers, and
services a model fault from a real virtio-blk device. What it does not
have is a policy: nothing in it yet decides what to keep, and that is
milestone M3 -- the one the whole argument turns on.

See [docs/status.md](docs/status.md) for what actually exists today. If it
is not in that file, it does not work.

## Documents

| Document | What it answers |
| --- | --- |
| [docs/PRD.md](docs/PRD.md) | What MLOS is for, and what "done" means |
| [docs/architecture.md](docs/architecture.md) | Kernel concepts, and the hardware/hypervisor reality we build on |
| [docs/design.md](docs/design.md) | Crates, syscalls, object table, device contracts |
| [docs/plan.md](docs/plan.md) | Milestones and the implementation sagas |
| [docs/status.md](docs/status.md) | Ground truth |
| [docs/layout-handoff.md](docs/layout-handoff.md) | What MLOS emits for the cross-repo visualization, and what it needs back |
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

## Development

MLOS boots in a virtual machine, never on bare metal. Apple Silicon is
the primary development host; a Linux/NVIDIA host is the second target,
where real GPU passthrough becomes possible.

Work is tracked with [agentrail](https://softwarewrighter.com) sagas.
`agentrail next` prints the current step.

---

Copyright (c) Software Wrighter LLC.
