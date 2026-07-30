# Snapshots are deletable, reversing "keep everything indefinitely"

Status: accepted

PRD §5.3/§5.5 originally decided to keep every Snapshot forever with no edit/delete in v1, treating the run history as an immutable audit trail. We're reversing the "no delete" half of that: any Snapshot can be deleted, for local storage/clutter management — this is a local-first, single-admin tool with no export path (CSV/Excel is an explicit non-goal) and no server-side retention policy, so manual deletion is the only lever against unbounded local SQLite growth.

Deletion requires a confirmation step (irreversible, unlike everything else in the diff model). If the deleted Snapshot was part of the current Baseline/Comparison selection, the dashboard falls back to the latest-vs-previous pair, the same rule used after a new run (ADR-0010) — a selection referencing a gone Snapshot always resyncs to latest-vs-previous rather than erroring. This doesn't contradict the Snapshot's "immutable" definition in `CONTEXT.md`: immutability means a Snapshot's content is never edited after the fact, not that its existence is permanent.
