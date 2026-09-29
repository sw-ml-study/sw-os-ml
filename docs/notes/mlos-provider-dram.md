# mlos-provider-dram

The resident tier as a provider: a bounded window of physical memory whose
`read` is a copy. Providers layer; one of the crates allowed to hold
`unsafe`. Linked from `crates/mlos-provider-dram/src/lib.rs` and
`bounds.rs`.

## Why a provider for memory that is already resident

Barely a provider, and that is the point. Its `handle` is a physical
address and its `read` is a copy, so it is the one provider that cannot
fail for reasons of its own. It exists because everything above it should
not have to know that resident memory is a special case: a fault on a
`Warm` object and a fault on a `Cold` one take the same path, and only
the cost differs.

`unsafe` lives here because reading from a physical address is exactly
what this crate is for (AGENTS.md, "Hard constraints"). Its one `unsafe`
block is the copy, and its `// SAFETY:` comment points at the bounds proof
that precedes it.

## Bounded on purpose

A provider that will read any address it is handed turns a corrupt table
entry into an arbitrary memory read; one that knows its own extent turns
the same entry into an error. `Dram::new` is `unsafe` because the caller
is promising the range is mapped, readable and reserved for object storage
for the provider's lifetime; everything after that is checked.

`bounds::within` is its own module because it is the only thing here that
can be wrong in a way that matters. Every arithmetic step is checked. The
inputs come from a table an object's own metadata populated, and treating
that as trusted is how a wrong `size` becomes a read of somebody else's
memory.

## Cost is reported as zero

Not literally free: a resident read still costs a cache miss. But the
number exists so eviction can compare tiers, and against three
milliseconds of NVMe the difference is noise. Reporting a real figure here
would be false precision.
