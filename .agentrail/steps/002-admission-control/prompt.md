Admission control: `ml_session_create` refusing is a normal outcome
(`docs/design.md` s.5.3; ladder rung L6 in `docs/architecture.md` s.5). A
budget admits a session only if the contracts already admitted and the
new one can all be honoured against it. Say what "can be honoured" means
in bytes -- the resident ceilings, the KV a session of its context will
hold -- and in latency, using what the G5 report measured: a ceiling costs
the other sessions reads. `Ss` becomes a capacity figure here, sessions a
budget admits, where M4 measured it at a fixed count. Simulator first,
kernel to the integer, `session new` refusing from the shell with the
reason.
