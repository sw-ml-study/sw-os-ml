# Notes, one per crate

Each crate's design reasoning and the lessons its bugs taught, kept out of
the source so the code can hold only the invariant and a link back here.
The rule is in [AGENTS.md](../../AGENTS.md) ("Comments: the invariant in
the code, the reasoning in the notes"); the layers are the ones
[anatomy.md](../anatomy.md) draws. A new crate gets its note in the same
commit as its first module.

## Hardware

- [mlos-hal](mlos-hal.md): The hardware abstraction layer: one trait, four methods, declarations only.
- [mlos-hal-aarch64](mlos-hal-aarch64.md): aarch64 platform support: the entry point, the `Image` header, the image extent, and the generic timer.
- [mlos-hal-x86-64](mlos-hal-x86-64.md): The x86-64 entry point, and the few facts about the CPU only the architecture can answer.
- [mlos-mmu-aarch64](mlos-mmu-aarch64.md): aarch64 boot translation tables: an identity map from a single level-1 table of 1 GiB blocks, then the MMU switched on.
- [mlos-trap-aarch64](mlos-trap-aarch64.md): aarch64 exception vectors: faults are captured, reported and stop; IRQs save the interrupted context, dispatch, and return.
- [mlos-trap-x86-64](mlos-trap-x86-64.md): x86-64 exceptions and interrupts: the IDT, one entry stub per vector, and the report a fatal fault prints.
- [mlos-gic-aarch64](mlos-gic-aarch64.md): A GICv3 interrupt controller driver: distributor, this CPU's redistributor, and the CPU interface.
- [mlos-apic-x86-64](mlos-apic-x86-64.md): The x86-64 interrupt controllers and clocks: the local APIC and its timer, the IOAPIC, the legacy 8259 PIC (silenced), and the TSC as a clock.
- [mlos-kernel](mlos-kernel.md): The microkernel binary: the entry point, bring-up, the boot banner, and the interrupt handlers.
- [mlos-kernel-x86-64](mlos-kernel-x86-64.md): The x86-64 half of `mlos-kernel`: what `boot.rs`, `banner.rs` and `handlers.rs` are for aarch64.
- [mlos-machine](mlos-machine.md): The machine, as the device tree describes it: memory, CPUs, console, interrupt controller, virtio slots, boot arguments, and where the blob itself sits.
- [mlos-pvh](mlos-pvh.md): What the PVH loader hands an x86-64 guest, parsed from bytes: the `hvm_start_info` header, its e820 memory map, and the kernel command line.

## Devices

- [mlos-virtio](mlos-virtio.md): virtio over MMIO: the transport and the split virtqueue that every virtio device driver rides on.
- [mlos-virtio-blk](mlos-virtio-blk.md): A virtio block device: read-only, one sector per request.
- [mlos-virtio-console](mlos-virtio-console.md): A virtio console, transmit only.
- [mlos-pl011](mlos-pl011.md): The Arm PL011 UART: polled transmit, interrupt-driven receive.
- [mlos-uart16550](mlos-uart16550.md): The 16550 UART reached through x86 port I/O: the x86-64 guest's console, COM1 on QEMU `microvm`.
- [mlos-fdt](mlos-fdt.md): A minimal reader for the flattened device tree: it walks the blob, it does not build a tree.
- [mlos-device](mlos-device.md): The device traits an MLOS platform implements: a console, an interrupt controller, a timer.
- [mlos-console](mlos-console.md): Choosing a console: PL011 or virtio, decided once at boot from the device tree and `/chosen/bootargs`.

## Objects

- [mlos-abi](mlos-abi.md): The MLOS ABI: everything that crosses the syscall boundary, with a fixed layout that is tested rather than trusted.
- [mlos-objtab](mlos-objtab.md): The ML object table: a fixed-capacity map from `ObjectId` to what the kernel knows about that object.
- [mlos-provider](mlos-provider.md): The `Provider` trait: where an ML object's bytes can come from, and what getting them costs.
- [mlos-provider-dram](mlos-provider-dram.md): The resident tier as a provider: a bounded window of physical memory whose `read` is a copy.
- [mlos-arena](mlos-arena.md): A fixed region with a coalescing free list: where a faulted-in object is put, and taken out of.
- [mlos-stream](mlos-stream.md): A declared sequence of objects and where the workload is in it: the mechanism by which a transformer hands the operating system its own future.
- [mlos-objman](mlos-objman.md): The model object manager: owns the object table, the arena resident objects live in, and the fault path between them.

## Policy

- [mlos-policy](mlos-policy.md): What to throw away when memory runs out: the `Policy` trait, the known-next-use policy this project exists to test, and the three baselines it is measured against.

## Observation

- [mlos-metrics](mlos-metrics.md): Running per-class counters and the report read out of them: what MLOS counts instead of throughput.
- [mlos-events](mlos-events.md): Residency transitions recorded as they happen, in a fixed ring the shell prints as JSON Lines.
- [mlos-layout](mlos-layout.md): The `sw-ml-study.system-layout` document: a columnar table of spaces and regions that a visualizer draws.
- [mlos-snapshot](mlos-snapshot.md): The running object store, streamed out as a `sw-ml-study.system-layout` document from inside the kernel.
- [mlos-spaces](mlos-spaces.md): MLOS's names for its layout spaces, its stable region ids and its region vocabulary, shared by both layout emitters.

## Shell

- [mlsh](mlsh.md): The MLOS inspector shell: the verbs a person or a boot script uses to poke the object manager and read back what it did.
- [mlos-line](mlos-line.md): One line of input, in a fixed buffer.
- [mlos-queue](mlos-queue.md): A byte queue from an interrupt handler to the loop it interrupted.
- [mlos-lab](mlos-lab.md): The one live object manager: a synthetic model held in a static arena, read from whatever device this machine turned out to have, swept and replayed from the shell.

## Host tools

- [mlos-cli](mlos-cli.md): `mlos`: build, run and diagnose MLOS from the host.
- [mlos-sim](mlos-sim.md): Replays an access trace against a policy under a budget, and counts what it cost.
- [mlos-trace](mlos-trace.md): An access trace: what a workload asked for, in order, and nothing about what the system did about it.
- [mlos-workload](mlos-workload.md): A decode loop, generated as an access trace: several sessions sweeping a model's weights and re-reading their own KV cache.
- [mlos-synth](mlos-synth.md): The synthetic model: its shape, the ids and metadata of its objects, and the tiers they come from.
- [mlos-image-map](mlos-image-map.md): MLOS's built artifacts as a `sw-ml-study.system-layout` document, plus the host side of lifting the runtime document and event stream off a captured console.
- [mlos-elf](mlos-elf.md): A minimal ELF64 section-header reader: where the kernel's sections land in memory, read from the linked file.
