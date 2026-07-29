# Persist GroupMembershipStatus at collection time, ahead of consumption

Status: accepted

`GroupMembershipStatus` (PD-1/PD-4: whether a group's membership was resolvable in a given Snapshot) isn't read by the diff engine in v1 — it exists for future visibility-flap detection (PD-1 user story 17, deferred). We considered leaving it out of the SQLite schema until whichever future ticket actually consumes it, keeping the schema minimal.

We decided to persist it starting with the first ticket that writes real Snapshots, not later. The PRD's retention model is "keep everything indefinitely, no pruning" (§5.3) — Snapshots are immutable and never recomputed from source. If the column isn't written now, every Snapshot taken before the future consuming ticket lands is permanently missing this data; it cannot be backfilled, since Bitbucket only reflects current state, not the state at each past Run. Writing an unread column is a minor schema cost; losing the ability to ever reconstruct this data for historical Runs is not reversible.
