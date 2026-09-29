# mlos-abi

The MLOS ABI: everything that crosses the syscall boundary, with a fixed
layout that is tested rather than trusted. Foundation layer; the one crate
both the kernel and userspace depend on. Linked from
`crates/mlos-abi/src/lib.rs`, `class.rs`, `error.rs` and `object.rs`.

## Why its own crate

It is the only crate both the kernel and userspace depend on, so it is
built for the bare targets on every run: if it ever stops being
`no_std`-clean, the ABI has quietly grown a host dependency. It is
`forbid(unsafe_code)` because an ABI definition that needs `unsafe` to
describe itself is an ABI that will be got wrong somewhere.

## Why `ObjectId` is structured, not opaque

FPGA gateware decodes the id directly (`docs/design.md` s.8.2). A handle
table would cost the ML-MMU a lookup on every translation; fixed bit
positions cost it a shift and a mask. That is the whole reason the layout
was fixed at step 002 rather than settled later, when moving it would be
expensive.

```text
 63    56 55        40 39        24 23        8 7      0
+--------+------------+------------+-----------+--------+
| class  |   model    |   layer    |  tensor   |  tile  |
+--------+------------+------------+-----------+--------+
   8 bits    16 bits      16 bits     16 bits    8 bits
```

The inner `u64` is public because that is exactly what crosses the
syscall boundary and what sits in an ML-MMU translation table entry.
Hiding it behind accessors would imply an invariant the type does not
have: a raw value arriving from userspace is arbitrary until
`ObjectId::class` accepts it.

`Fields` is returned as a group rather than through five accessors
because that is how the hardware reads them: gateware latches the whole
word and slices it once. Software that mirrors the hardware's shape stays
in agreement with it. `fields` is infallible (every bit pattern is a valid
set of fields); only the class byte can be malformed, so `class` is the
one fallible decode.

`const` assertions pin the word to eight bytes and the four shifts to
adjacent, non-overlapping fields, so a renumbering that broke the layout
fails to compile rather than to translate.

## The tile field

`tile` is zero for tensor-granular objects. Tile granularity is open
question Q1 in `docs/PRD.md`, to be answered by measurement at M3; the
field costs nothing to carry until then, and adding it later would mean
renumbering everything above it.

## Why zero is not a class and not an error

An all-zero `ObjectId` is the value uninitialised memory and a lazy caller
both produce. With zero absent from `ObjectClass`, it fails to decode
instead of naming object 0 of model 0. Zero is likewise absent from
`Error`: it is success, and never an `Error`.

Discriminants of both enums are part of the ABI and are never reused or
renumbered. For `ObjectClass` the constraint is physical: the class byte is
decoded in gateware, where a renumbering means a bitstream rebuild.

## Why the class exists

The class is what makes a model fault more useful than a page fault: it
selects which residency policy applies, and it is the axis every metric in
`docs/PRD.md` s.5.2 is broken down by. "40,000 faults" is meaningless;
"38,000 of them cold KV blocks" is a diagnosis.

Per class, the properties the policies lean on:

- `WeightTile`: dense layer weights. Immutable, large, consumed
  sequentially, shareable across every session using the model.
- `Scale`: quantization scales. Tiny, hot, effectively always resident.
- `Expert`: one MoE expert's weights. Immutable, selected
  probabilistically by the router; the class that makes prefetch a
  distribution rather than a guess.
- `KvBlock`: a block of one session's KV cache. Mutable, grows with
  context, partly compressible, partly droppable.
- `Activation`: intermediate activations. Transient, and usually cheaper
  to recompute than to reload; the class that makes `DISCARD` a
  legitimate eviction verb.
- `EmbedBlock`: a block of an embedding table. Immutable, randomly
  accessed.
- `RagBlock`: a retrieved document block. Immutable, semantically
  addressed.
- `Adapter`: a LoRA-scale adapter. Small, and the only weights that are
  mutable before training enters scope.

`ObjectClass::ALL` exists so accounting can be per-class without anywhere
keeping a second list that drifts from this one; adding a class there is
the only edit a new class needs.

## Session scope is a function, not a convention

Session-scoped classes (`KvBlock`, `Activation`) reuse `Fields::model` as
a session id; the class is what disambiguates the two readings. Getting
this wrong would let one session's KV alias another's, so
`ObjectClass::is_session_scoped` is a single function every caller asks
rather than a rule each caller re-derives.

## `Refused` is not a failure

`Error::Refused` is admission control declining. Refusing a session the
system cannot serve within its contract is the designed outcome, and is
what MLOS does instead of overcommitting and thrashing (`docs/PRD.md` F4).
