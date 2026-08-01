# Roster export: CSV only, carving out the PRD's no-export non-goal

The PRD's non-goals list ruled out Excel/CSV export outright, on the premise that the dashboard should fully replace the old spreadsheet-based workflow rather than maintain two output paths. That premise held as long as "export" meant a recurring reporting mechanism. It doesn't cover a different, narrower need: producing a one-off file to hand to a reviewer (e.g. attached to an email) or to keep as an offline archive copy. The dashboard stays the source of truth for ongoing audits; Export is just a snapshot artifact taken out of it on demand.

An HTML export (mirroring on-screen diff state, filters, and app styling) was considered and dropped: faithfully reproducing what's on screen would require either forcing every repo card open and serializing live DOM (which strips interactive edit controls that don't belong in a static file, and mutates the user's actual UI state to do so) or a second, dedicated markup generator kept in sync with `RepoCard.svelte`/`RosterRow.svelte` by hand — both too much machinery for a first cut. CSV alone covers the stated need (a plain data file for archiving or attaching) at a fraction of the cost.

CSV is always a plain, single-Snapshot roster (`repo_project, repo, username, display_name, access_type, scope, permission, run_at`) — no diff semantics, no filter/view-mode state, no notion of "what's on screen." It's raw archival data, not a view export, so it stays complete and diff-free even when the dashboard itself is showing a diff between two Snapshots. In diff mode, it exports the Comparison Snapshot (the second Run-pair selector), not the Baseline.

Triggered from a single, whole-workspace-only "Export" action in the head bar (no per-repo action, no format menu, since there's only one format). Saving goes through a native OS "Save As" dialog (no app-managed export folder), pre-filled with a filename encoding the workspace and the exported Run's timestamp. The app never sends email itself — Export's job ends at producing the file.

## Considered options

- **HTML export mirroring on-screen diff/filter state** — considered and rejected (see above); may be revisited later behind its own ADR if the need for a shareable formatted view resurfaces.
- **Per-repo export actions** — rejected for v1; a Run's unit is the whole workspace, and a reviewer audience wants the whole roster, not a repo at a time.
- **Auto-save to a fixed export folder** — rejected in favor of a native Save As dialog, since the destination (an email draft, an external archive location) is decided per export, not fixed.

## Consequences

- The PRD's non-goal wording changes from an absolute "no Excel/CSV export" to "no *recurring* Excel/CSV reporting path" — a future reader comparing the non-goals list against the Export feature won't see a contradiction.
- `GrantScope` (Repo vs. Project, ADR-0012) is included as its own CSV column even though it predates this feature in the codebase but not in the original PRD-era data model description — needed for the CSV to be a faithful, lossless copy of what the roster tree actually carries.
