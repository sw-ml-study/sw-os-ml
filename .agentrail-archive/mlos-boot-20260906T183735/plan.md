# MLOS Boot

Vision: one kernel binary that reaches a console and a timer tick as an
aarch64 guest, under two different hypervisors, from one source tree.

Nothing ML-shaped exists yet. This saga earns the right to write the
interesting parts: until MLOS boots, every claim in docs/architecture.md
is untested prose. M1 turns the document set into a running program that
prints one honest line.

Gate G1 from docs/PRD.md: MLOS starts from firmware, initializes memory,
brings up a timer and a console, and reaches a shell prompt -- under
QEMU/HVF and under Virtualization.framework, from the same source.

Deliberately NOT in this saga: x86-64 (that is M6), userspace, scheduling
beyond a single kernel thread, and any ML concept whatsoever. The object
table is M2. Resisting that is the point.

1. **workspace** -- Cargo workspace on Rust 2024, both bare targets
   building an empty kernel, sw-checklist green. Turns the pre-commit
   gate on for the first time.
2. **abi-skeleton** -- mlos-abi: error codes, ObjectId bit layout and
   its layout test. Written now, not at M2, because the ML-MMU register
   contract in docs/design.md depends on these exact bit positions and
   changing them later is expensive.
3. **hal-trait** -- mlos-hal: the four-method Hal trait, BootInfo,
   Console/Timer/IrqController traits. No implementation.
4. **aarch64-entry** -- _start at EL1, DTB parse, page tables, MMU on,
   stack switch. The first code that must run on real silicon.
5. **console-timer** -- PL011 console, GICv3, ARM generic timer. The
   first printed line and the first interrupt.
6. **cli-run** -- mlos build / mlos run --host hvf|tcg / mlos doctor.
   doctor earns its place: host prerequisites differ sharply between
   the Mac and the Linux box.
7. **uefi-and-vz** -- UEFI image path; boot under
   Virtualization.framework. Requirement N2 (two hypervisors per arch)
   satisfied, or it never will be -- direct -kernel boot works too
   easily to leave this for later.
8. **ci-tcg** -- deterministic TCG boot test in CI. A boot failure that
   reproduces identically every run is what makes kernel debugging
   tractable.

Parked until this saga lands:
- x86-64 HAL. One architecture working beats two half-working.
- Anything from docs/plan.md M2 onward.
