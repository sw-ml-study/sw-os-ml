Session objects. `docs/architecture.md` s.3.4 and `docs/design.md` s.5.3:
a session is a kernel object with an id, a contract, a budget, and
ownership of its KV blocks; `ml_session_create` may refuse and
`ml_session_destroy` releases what the session owned. Today a session is
a `u16` tag on an access and an `owner` field on metadata, and nothing is
created or destroyed. Make it a record in a `no_std` crate the kernel and
the simulator both use, give it a lifetime (create, destroy, and eviction
of its objects on destroy), and drive it from the shell. Contracts beyond
what this milestone needs are M5; state which field is used here.
