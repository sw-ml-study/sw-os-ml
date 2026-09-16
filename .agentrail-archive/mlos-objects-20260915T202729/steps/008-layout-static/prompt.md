Emit build/storage-layout.json: MLOS's static layout in the COLUMNAR contract
pinned by ../../sw-ml-study/sw-mlpl/docs/storage-layout-viz.md, so MLOS becomes
the second producer of a format sw-tos already emits and sw-mlpl already parses.

Contract, verbatim from that doc -- do not invent a variant. Header: schema
"sw-ml-study.system-layout", version 1, provenance {producer: "mlos", revision:
<git rev>}. Index-aligned space_* columns of length S with `spaces` as the key
column; index-aligned region_* columns of length N: region_id, region_space,
region_kind, region_name, region_owner, region_start, region_length, and the
three word-count columns (0 for MLOS -- we have no 24-bit image words). An edge
table rel_kind/rel_from/rel_to, which may be empty at this step. Byte offsets
and lengths throughout.

Three spaces, all figures read from the artifacts that already produce them
rather than recomputed:

  disk   -- the image `mlos image disk` builds. block 512 (mlos_virtio_blk::SECTOR),
            capacity from the built file. One region per weight tile, placed at
            Disk::sector_of, named by layer/tensor, plus an explicit free tail.
  dram   -- the object arena. block 16 (Arena's alignment), capacity from the
            arena reservation. Statically it is entirely one `free` region: that
            is the point, and step 009 fills it in.
  sysram -- the physical map: kernel text/rodata/data/bss from the built ELF,
            the arena reservation, and free. block 4096.

Regions must TILE each space -- no gap, no overlap, totals equal to capacity --
so emit explicit free and padding regions and keep padding distinct from free
where alignment actually costs something.

MLOS needs columns sw-tos does not, and the contract permits extras: region_tier
(hot/warm/cold/stream/archive), region_class (the ObjectClass name), and
region_object_id (the u64 ObjectId as a string -- it does not fit a JSON number
safely). Take every one of these from mlos-abi/mlos-objtab/mlos-synth, never
from a second hand-written table.

region_id must be a STABLE integer, the same id for the same thing in every
snapshot and in the runtime file step 009 emits -- the viewer picks and
cross-highlights on it. Derive object-backed ids from the ObjectId; reserve a
small low range for structural regions (kernel, arena, free). Write down the
derivation where the next reader will find it.

Host-side, in mlos-cli: `mlos layout --out build/storage-layout.json`. No
emulator in this step -- that separates contract correctness from anything that
can only fail under QEMU.

Add a contract test that checks alignment, tiling, totals and id stability, not
the numbers, and mutation-check it: a dropped column, a one-region gap, a
stretched capacity, a reused id must each fail it. Register it in the gate.
