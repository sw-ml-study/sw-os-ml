Emit build/runtime-layout.json: the same columnar contract, but for the RUNNING
system -- which for MLOS is the whole point. The static file from step 008 says
what exists; this one says what is resident, and the difference between them is
the thesis.

An `mlsh` verb -- `layout` -- writes the columnar JSON to the console, and
`mlos run --capture` lifts it out to build/runtime-layout.json. Same schema,
same version, same provenance shape, producer "mlos", and crucially the SAME
region_id derivation as step 008 so a disk region and the arena region holding
its bytes can be joined across the two files.

What changes relative to the static file: the `dram` space now has one region per
resident object, placed at the arena handle the object manager actually gave it,
followed by the unused tail as `free` -- the high-water mark is then visible as
the boundary rather than asserted. Take base, handle and occupancy from the
Arena and the ObjectMeta, not from a recomputation.

The runtime columns the object table already holds, and no page-based system
could emit: region_state (resident/evicted/never), region_tier, region_reuse
(reuse_count), region_next_use (never | distance N | probability P -- render the
three-way NextUse as a string, do not collapse it to a number, for the reason
NextUse's own doc comment gives), and region_cost (reload_cost, ns).

Fill the edge table: `backs` from each disk region to the arena region holding
its bytes, for every resident object. That is MLOS's version of sw-tos's
catalog->extent->allocation chain, and it is what an "explain selected object"
view consumes: tile on disk -> model fault -> arena placement.

Emitting must not perturb what it measures: `layout` must not fault objects in,
and must not disturb the metrics counters. Say so in a test.

The console is a 115200-baud pipe, so mind the size: 136 objects of a dozen
columns is fine, and note in the summary where the ceiling is for anyone who
scales the model up.

Contract test as in step 008, plus: a layout taken before any acquire and one
taken after a sweep must agree on region_id for every object that appears in
both, and must differ in region_state for at least one. Register it in the gate.
