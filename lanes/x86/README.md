# x86-64 lane

**Done and archived** (2026-09-27): saga `mlos-x86-64`, all nine steps, is
in `.agentrail-archive/mlos-x86-64-20260927T000059/` here. What the x86-64
guest does is in [docs/status-x86-64.md](../../docs/status-x86-64.md).

The agentrail saga for `mlos-x86-64` (docs/plan.md) lives here, not in
the root `.agentrail/`, so it can run alongside the aarch64 lane's active
saga without either branch touching the other's saga files. Drive it with
`--saga`:

```bash
agentrail --saga lanes/x86 next
agentrail --saga lanes/x86 begin
agentrail --saga lanes/x86 complete --summary "..." --reward 1 --actions "..."
git add lanes/x86/.agentrail && git commit -m "step NNN-<slug>: saga metadata"
```

Everything else in AGENTS.md applies unchanged, with `lanes/x86/.agentrail/`
wherever it says `.agentrail/` (agentrail's own reminders still print the
root path; ignore that).

**Conflict discipline for this lane.** x86-64 code goes in new files:
new crates (`mlos-hal-x86-64`, `mlos-kernel-x86-64`, ...),
`crates/mlos-cli/src/x86.rs`, `crates/mlos-cli/tests/boot_x86.rs`, and
`docs/status-x86-64.md` rather than rows in `docs/status.md`. Shared files
get the smallest possible hook, placed where the aarch64 lane is least
likely to be editing. Folding it back into shared locations is a later,
deliberate refactor, done when both lanes have merged.

When the saga is done: `agentrail --saga lanes/x86 archive`, then merge.
