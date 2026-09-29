share_count, maintained. `ObjectMeta::share_count` exists and is set from
a lease's pin and read by nothing. Make it true: the count of sessions
currently holding the object, kept by acquire and release, and a policy
input a shared object is worth more than a private one. Leases carry
intent (`docs/architecture.md` s.3.4: PIN, BORROW, STREAMING,
SPECULATIVE); say which of them count, and add a test that a shared
object outlives a private one under pressure.
