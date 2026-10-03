Contracts, made whole. `Contract { quality_floor, latency_ceiling,
resident_ceiling }` exists (M4) with the resident ceiling enforced by the
manager and the latency ceiling read by the scheduler; the quality floor
is a bare u8 nothing reads. Give each field a meaning the kernel can
account against: quality as the lowest `Precision` a session's objects may
be degraded to, latency and residency as they are, and a record per
session of what was actually delivered (the lowest precision reached, the
worst period, the peak resident bytes), so `ml_session_contract` can
answer `docs/design.md` s.5.3's question: what was delivered against what
was promised. Shared `no_std` code in `mlos-session`, reported by the
shell and the simulator alike.
