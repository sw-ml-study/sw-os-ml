# mlos-provider

The `Provider` trait: where an ML object's bytes can come from, and what
getting them costs. Providers layer; the only place that knows about
physical media. Linked from `crates/mlos-provider/src/lib.rs` and
`located.rs`.

## Why providers exist

Providers are the only place that knows about physical media. Above them,
an object has a class, a size and a next-use distance; below them, it is
bytes on some device. That separation is what lets the kernel move an
object between tiers while a consumer holds a handle to it, which is the
whole claim in `docs/architecture.md` s.1.

## `resolve` is deliberately absent

`docs/design.md` s.6 lists a `resolve` operation on providers. It is not
here. Resolution is the object table's job: an id yields an `ObjectMeta`,
which names the provider and carries the handle. Asking the provider to
resolve as well would be a second lookup answering a question already
answered, on the fault path, where there is least room for one.

## `Located` carries both the id and the handle

Providers need different halves of it. A block store needs the handle and
does not care what the bytes mean. A recompute provider needs the id: it
has to know which activation to rebuild, and that is exactly what class,
layer and tensor say. `Located::new` reads a table entry as a fetch
request so the two are never assembled by hand.

## Partial reads are normal

`Provider::read` returns how many bytes arrived, and fewer than asked is
not an error: an object may be larger than the buffer a caller is willing
to give, and a weight tile is consumed in pieces.

## `Cost` is two numbers

Latency and throughput behave differently: latency is paid once however
small the object, throughput scales with its size. An eviction policy
comparing a 2 MB activation against a 40 MB expert gets the wrong answer
from either number alone. `Cost::for_bytes` combines them into the one
number eviction actually compares, and that number is the reason `cost`
is on the trait at all: a policy that cannot ask what recovery would cost
is guessing, which is precisely what LRU does.

`bytes_per_ms == 0` means "no faster than instantly", which is what
resident memory costs; `for_bytes` then returns latency alone rather than
dividing by zero.

`cost` is a property of the medium, not of the object, so a provider
overrides it once with a constant rather than computing per call.

## `prefetch` defaults to nothing

The default does nothing, which is honest for a medium with no notion of
getting ready: resident memory cannot be prefetched, and pretending
otherwise would have policies counting hits that never happened.
