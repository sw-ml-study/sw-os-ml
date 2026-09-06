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

Pre-implementation. The architecture is written; the kernel is not.
See [docs/status.md](docs/status.md) for what actually exists today.

## Documents

| Document | What it answers |
| --- | --- |
| [docs/PRD.md](docs/PRD.md) | What MLOS is for, and what "done" means |
| [docs/architecture.md](docs/architecture.md) | Kernel concepts, and the hardware/hypervisor reality we build on |
| [docs/design.md](docs/design.md) | Crates, syscalls, object table, device contracts |
| [docs/plan.md](docs/plan.md) | Milestones and the implementation sagas |
| [docs/status.md](docs/status.md) | Ground truth |
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

(Do not paste a trailing `# comment` after these: interactive zsh does not
treat `#` as a comment, so it arrives as an argument. `mlos` will now say
so rather than passing it to QEMU.)

## Development

MLOS boots in a virtual machine, never on bare metal. Apple Silicon is
the primary development host; a Linux/NVIDIA host is the second target,
where real GPU passthrough becomes possible.

Work is tracked with [agentrail](https://softwarewrighter.com) sagas.
`agentrail next` prints the current step.

---

Copyright (c) Software Wrighter LLC.
