# Refresh advances the Baseline, not just the Comparison

ADR-0025 specified that a successful Refresh "auto-selects its new Snapshot as Comparison exactly as a Run does," but left the Baseline untouched. In practice this produced a misleading pair: if the dashboard was showing A→B when the user staged and applied an edit batch against B, Refresh created C and left the pair at A→C. That conflates the batch's actual effect (B→C) with whatever unrelated drift already existed between A and B — the edit's own effect can be invisible or misattributed. A full Run doesn't have this problem: `RunPanel.svelte` drops the `baseline`/`comparison` URL params entirely, and `+page.ts`'s load function defaults an absent pair to `previousId → latestId`, so Run always advances both Snapshots by one step. Refresh should do the same.

On a successful Refresh, the new Baseline is the pre-refresh Comparison — concretely, `dialogSourceSnapshotId`, the Snapshot the applied batch was staged against (the same value already passed as `refresh_snapshot`'s `sourceSnapshotId`). The pair goes from A→B to B→C, isolating exactly what the batch changed. This is a pure frontend navigation change (`src/routes/dashboard/+page.svelte`'s `refreshAffected`); no backend or stored-Snapshot change is involved.

## Considered options

- **Leave Baseline untouched (status quo, per ADR-0025)** — rejected; this is the bug being fixed. Nothing about Refresh's transient, single-batch scope justifies pinning Baseline to whatever was selected before the edit was staged.
- **Prompt the user to confirm the new pair, or surface a "Baseline advanced to run N" notice** — rejected for consistency: a full Run silently repoints both Snapshots with no confirmation or notice, and Refresh should behave the same way. The new pair is visible in the roster view itself (`SnapshotsPanel`, `HeadBar`) without extra messaging.
- **Advance Baseline to the pre-refresh Baseline's own predecessor, or otherwise recompute a "latest two"-style default** — rejected; the correct new Baseline is specifically the Snapshot the batch was staged against, not a generic "one step back," since that's the one Snapshot guaranteed to be the immediate ancestor of the batch's effect.

## Consequences

- Updates ADR-0025's Refresh description and the `CONTEXT.md` Refresh glossary entry, both of which said "Comparison" only; ADR-0025 gets a superseded-in-part status note pointing here rather than being rewritten in place.
- If Baseline and Comparison were already the same Snapshot before the batch was applied (a pure roster view, no diff), the new pair is unchanged in kind — Baseline (unchanged) → new Snapshot — since the pre-refresh Comparison and pre-refresh Baseline are the same Snapshot.
