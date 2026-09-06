# MLOS Foundations

Vision: a new operating system, written from scratch in Rust 2024, that
treats machine-learning state -- weights, KV cache, MoE expert streams,
query context, activations, embeddings, adapters -- as its primary
virtualized resource, the way conventional operating systems treat
address spaces and pages.

Not a modified BSD or Linux. Not a framework. A kernel whose central
abstraction is the ML object, not the page.

The constraint that shapes everything: today's CPUs virtualize pages,
not tensors. x86-64 and Apple Silicon MMUs know nothing about a KV
block or an expert. So the first implementation is a SOFTWARE
model-object manager riding on page hardware, structured so that when
an ML-MMU exists (first as an FPGA add-in card, later perhaps in
silicon) the kernel changes its provider, not its abstraction.

Development happens in a VM, never on bare metal. Apple Silicon first
(the machine on the desk), Linux/NVIDIA second (where the GPUs are).

This saga is the PRELIMINARY one: it produces the written architecture,
not the kernel. Implementation sagas follow, and are proposed in
docs/plan.md.

1. **repo-bootstrap** -- AGENTS.md briefing, .gitignore, docs skeleton,
   Software Wrighter conformance gates (Rust 2024, sw-checklist).
2. **prd** -- docs/PRD.md. What MLOS is for, who it serves, what
   "done" means for the proof of concept, what is explicitly out of
   scope.
3. **architecture** -- docs/architecture.md. The five kernel concepts
   (ML_OBJECT, PROVIDER, TIER, LEASE, STREAM), the model fault, the
   parameter-major scheduler, and the platform survey: Apple Silicon
   vs x86-64, hypervisor choice, GPU passthrough reality, the FPGA
   ML-MMU roadmap.
4. **design** -- docs/design.md. Crate layout, no_std kernel, the
   syscall surface, the ML object table, the virtio/host-device
   contract, the FPGA register interface, and how each piece is tested
   without hardware.
5. **plan-and-status** -- docs/plan.md (milestones, the follow-on
   implementation sagas) and docs/status.md (where we actually are).

Parked, deliberately, until the kernel boots and holds an object table:
- Training state (gradients, optimizer state, checkpoints). Inference
  and LoRA-scale mutable state first.
- The AI/agent operating environment (durable agent jobs, virtual
  context, tools as capabilities). That is a sibling project sharing
  the substrate, not part of this one -- see docs/research.txt s.1-34.
- Real FPGA gateware. The ML-MMU is specified and emulated here;
  emufpga is where it gets built.
- Bare-metal boot on real hardware.
