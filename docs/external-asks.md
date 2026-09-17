# What MLOS needs from other repositories

Status: LIVE. Read with [plan.md](plan.md) for when each is needed and
[status.md](status.md) for what already works. Every entry says what is
wanted, where it would live, why MLOS cannot reasonably do it itself, and
**what MLOS does if the answer is no** -- because an ask with no fallback
is a plan with a single point of failure.

Nothing here blocks MLOS today. One ask (emufpga A1) blocks M3 step 010,
which is seven steps away and is about realism rather than about whether
the measurement can be made at all.

## How to read an ask

- **Blocking** -- a named MLOS step cannot proceed. There is a date on it.
- **Wanted** -- MLOS can proceed without it, but will do something worse:
  duplicate work, or invent a figure it should have been given.
- **Later** -- not needed yet; recorded so it is not a surprise.

MLOS's own side of each relationship is in
[§6, What MLOS provides back](#6-what-mlos-provides-back). Several of
these are trades, not requests.

## 1. emufpga

The most substantial relationship, in both directions. emufpga is the
research vehicle for streaming weights off sequential storage; MLOS is
the operating system that would schedule such a stream. They need each
other's artifacts more than either needs the other's code.

### A1. An order file and sidecar from a real model -- BLOCKING (M3 step 4)

**What.** The two small text files that come out of an import: the
`spm-order` order file with its `[rotating]` and `[resident]` sections,
and the `spm-import` sidecar listing `stream / name / rows / cols /
elements` with its `rotating-streams` count. **Not the weights** -- MLOS
does not want a multi-gigabyte `.spm` and will never read one. Kilobytes
of text.

**Where.** Committed in emufpga, beside the existing
`components/format/crates/spm-file/tests/golden/tiny.spm` fixture, for
whatever real model is most convenient. `scripts/extract-checkpoint`
already reads `.safetensors` as well as `.pt`, so the smallest published
safetensors model anyone has is enough.

**Why.** M3's whole claim is that a transformer's access order is knowable
in advance. Testing that claim against an access order MLOS invented would
be circular. The order file is a real model's real consumption order, and
it is the difference between a measurement and an assumption --
[architecture.md](architecture.md) s.12 names hand-written traces as a
risk for exactly this reason.

**Why not MLOS.** MLOS has no checkpoint reader and should not grow one.
The import is emufpga's existing, tested job.

**If no.** Use `tiny.spm`'s fixture shape, and mark every number in the
G4 report as resting on a fixture rather than a model. That is a real
weakening of the result and it would have to be stated in the report.

### A2. The consumption-order crates, consumable from outside -- WANTED (M3 step 4)

**What.** `spm-order`, `spm-layout` and `spm-walk` reachable from another
repository -- published, or an agreed path dependency, or vendored with a
note. `spm-walk::Cursor` in particular: it is the one place that knows
where a stream ends.

**Where.** emufpga's `components/format/`, unchanged in substance.

**Why.** `spm-order`'s own documentation says the first import got the
order backwards -- alphabetical gave `down_proj, gate_up_proj, o_proj,
qkv_proj`, the exact reverse of execution order -- and that a forward
sweep of such a file would have to seek backward, which the architecture
forbids. MLOS re-deriving that order from a sidecar is MLOS lining up to
make the same mistake, in a repository with no test that would catch it.

**If no.** MLOS parses the order file's `[rotating]`/`[resident]`
sections itself, which is a dozen lines, and accepts that the two repos
now have two implementations of one rule.

### A3. KV growth per token -- WANTED (M3 step 4)

**What.** For a given model and client, how many bytes of KV are added per
token, per layer. `spm-kv::KvCache::new(layers, context, width)` and
`bytes()` already compute this; the ask is a number or a documented
formula, not an API.

**Why.** M3 step 4 needs a generative trace where KV blocks accumulate and
are re-read, because that is the workload on which LRU has a fair chance
to be right. If MLOS invents the KV block size, it chooses how much
pressure its own thesis is tested under -- which is marking its own
homework.

**If no.** Derive it from the sidecar's attention tensor widths and say
plainly in the G4 report that the KV figure is derived rather than taken
from a serving implementation.

### A4. MoE router decisions -- LATER (M5, and M3 if MoE lands sooner)

**What.** When MoE arrives in emufpga, a record of the router's top-k
choice per token per layer.

**Where.** emufpga's stream or serve components. Nothing implements MoE
today; `spm-stream-metrics` mentions it in documentation only.

**Why.** `NextUse::Probability` exists in MLOS's object table and nothing
will ever write it without this. It is the half of the thesis that is
*not* a dense sweep: a router yields a distribution, not a distance, and
a policy may act on the two differently. Without real router output, the
probability path is untestable and MLOS should say so rather than
demonstrate it against a distribution it made up.

**If no.** The G4 report covers the `Distance` half only, and says the
`Probability` half is unmeasured. That is honest and it is a smaller
claim than the PRD makes.

### A5. Read the ML-MMU register contract -- LATER (M10)

**What.** Acknowledgement, and a review of the `CAPS` bits and the
descriptor ring. MLOS owes emufpga this artifact and it exists now, at
[design.md s.8](design.md#8-the-ml-mmu-register-contract): BAR0 layout, a
16-byte translation-table entry, and one descriptor kind with six
opcodes.

**Why now, when it is needed later.** There is no reference to `ML-MMU`,
`mlmmu` or gateware anywhere in emufpga, so the likeliest state of the
world is that this contract was written and never read. A contract only
one side has seen is not a contract. Finding out at M6 that the `ROUTE`
opcode is unbuildable would waste the four milestones in between.

**If no.** MLOS builds the Gen 0 QEMU device model against its own
contract and the gateware question stays open. The ML-MMU is M10 and
deliberately last -- hardware should accelerate what has been shown to
work rather than what was hoped would -- so this blocks nothing except
the contract being worth anything.

### A6. Nothing, for now -- the relationship changed shape

The 2026-09-17 reframing ([plan.md](plan.md)) moved MLOS away from
driving hardware and toward controlling host resources through narrow
interfaces. emufpga's streaming engine is a compute provider under that
model, reached the same way a CUDA host service would be, rather than a
device MLOS drives. That is a better fit for both projects and needs
nothing from emufpga until M10.

## 2. sw-mlpl

Detail, with the exact closed sets, is in
[layout-handoff.md](layout-handoff.md). Summarised here so this file is
the one index of outstanding asks.

### B1. Three palette rows -- BLOCKING (their renderer, not MLOS's work)

**What.** `reserved`, `rodata`, `weight-tile` in `u:default_palette()`.

**Why.** `select_rows` is strict: a kind with no row is an error, not a
default colour, so MLOS's committed samples do not render at all today.
This is the entire blocker and it is three lines.

**If no.** MLOS renders through sw-tos's vocabulary only, which means
dropping the ML object classes -- the part that makes the picture worth
drawing.

### B2. Confirm empty arrays parse -- WANTED

**What.** That `parse_json` accepts `"rel_kind": []`.

**Why.** MLOS's static document has no edges, because nothing is resident
in a build artifact. The columns are emitted as empty arrays rather than
omitted. It is the one shape MLOS could not test against their
interpreter from here.

**If no.** MLOS omits the columns instead, which breaks consumers that
read them unconditionally -- strictly worse, but knowable.

### B3. The remaining object classes, when they appear -- LATER

`scale`, `expert`, `kv-block`, `activation`, `embed-block`, `rag-block`,
`adapter`. Not needed until MLOS registers them. `expert` and `kv-block`
are the two worth distinct colours: one is selected probabilistically and
one grows with a session.

## 3. demo-extensions

### C1. Two spaces with edges between them -- QUESTION

Does `set_boxes` plus `set_highlight` express "draw these two spaces
apart, and these 32 lines between them"? MLOS's runtime document carries
`backs` edges from a stored tile to the arena region holding its bytes,
and that relationship is the picture MLOS exists to show. Their call
whether it wants a primitive.

### C2. A time axis -- QUESTION

`runtime-events.jsonl` is a stream of residency transitions. Animating it
is what makes an ML object store look different from a flash image.
Nothing in the proposed surface obviously drives an animation, and that
may be the right answer for now.

### C3. Scale within one space -- OURS TO FLAG, THEIRS TO FIX

MLOS's `sysram` space is 512 MiB of which about 509 MiB is one free
region, against a 115 KiB `.text`. Rendered linearly the kernel is a
hairline. Per-space block sizes already take the worst cross-space ratio
from 16384:1 down to 512:1; within `sysram` this needs a log scale or an
elided free tail. SWTOS does not hit it -- its flash is mostly used.

## 4. sw-tos

### D1. `space_used` and `program_*` -- QUESTION

Both are emitted and neither is in the pinned contract. Promote them, or
keep them as producer extensions like MLOS's `region_tier` and
`region_state`? A consumer cannot tell the difference today, which means
one written against SWTOS may read a column MLOS will never emit.

No other ask. MLOS is the second producer of a format SWTOS defined in
practice, and what MLOS did differently is written down in
[layout-handoff.md s.5](layout-handoff.md) so the divergences are visible
rather than silent.

## 4b. A host compute service -- LATER (M6)

Not a repository ask; a thing that has to exist. M6 needs something on
the host that answers "make this object resident in device memory" over
`virtio-ml-compute` and does the transfer with CUDA or Metal. It is
small -- a few hundred lines around an existing runtime -- and it is the
piece that lets MLOS reach a GPU without driving one.

Where it lives is open. It could be a new sibling repository, or it could
be part of this one as the only host-side component MLOS ships. Worth
deciding before M6 rather than during it.

## 5. demo-memory

### E1. What the eviction experiments measure -- WANTED (M3 step 6, M5)

**What.** Not code. The shape of what its `kv`, `attention` and
`retrieval` demos measure, and any policy candidate that turned out to
beat the obvious one.

**Why.** [plan.md](plan.md) s.4 lists demo-memory as a source of eviction
and retrieval policy candidates for M3 and M5, and that entry has never
been acted on. MLOS is about to write demand, FIFO, LRU and known-next-use
from first principles; if demo-memory has already found that some variant
matters, MLOS should know before rather than after.

**Why not MLOS.** MLOS can write the policies. What it cannot cheaply
acquire is the experience of having compared them, which is what that
repository is for -- it describes itself as a downstream forcing function
that files precise upstream capability requests, which is the same posture
this document takes.

**If no.** MLOS writes the three textbook baselines and says they are
textbook.

## 6. What MLOS provides back

| To | What | State |
| --- | --- | --- |
| emufpga | The ML-MMU register contract: BAR0, translation-table entry, descriptor ring ([design.md s.8](design.md#8-the-ml-mmu-register-contract)) | Written, apparently unread -- see A5 |
| sw-mlpl, demo-extensions, sw-tos | Three conforming sample artifacts under `examples/viz/`, with full sha256 digests, regenerable by `mlos layout` and `mlos runtime` | Shipped |
| sw-mlpl, demo-extensions | The closed vocabularies, the event shape, and the open questions ([layout-handoff.md](layout-handoff.md)) | Shipped, and checked by a test so it cannot drift |
| sw-tos | `mlos_layout::validate(&str)`: a contract checker that reads the rendered text rather than the types, mutation-checked six ways. Not MLOS-specific | Offered |
| sw-mlpl | A second producer's reading of the contract, so ambiguities show up as two implementations rather than one | Shipped |

## 7. How to answer

Anything here can be answered by editing this file, by a commit in the
other repository, or by telling the MLOS agent. What would help most is
an answer that says **no** where the answer is no: a fallback taken
deliberately costs less than one taken after waiting.
