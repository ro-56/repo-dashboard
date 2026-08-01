# Refresh: a targeted Snapshot copy-plus-fetch, offered as a complement to Apply — not a full Run

Applying a batch of Pending edits (ADR-0022) mutates live Bitbucket state, but until now the only way to see that reflected in the dashboard was a full Run — workspace-wide discovery plus a fetch of every repo and Project, even though the edit batch itself only ever touches a handful of them. Refresh is a new, separate action offered in the Apply results view: it creates one new Snapshot by copying every repo/project the batch didn't touch verbatim from the source Snapshot, and re-fetching live data only for the Refresh targets named by the batch's *successful* edits (Project-scope edits cascade to every repo the source Snapshot already records under that Project — Refresh never calls `list_repositories` to redetermine that set).

Refresh is deliberately narrow and transient: it's reachable only from the just-finished Apply's results view, targets only successful edits (failed ones stay staged for retry, per ADR-0022, and contribute no target), fires immediately with no confirm step (it only reads), clears any remaining staged edits and auto-selects its new Snapshot as Comparison exactly as a Run does, and is retired the moment that results view is dismissed — a full Run is the only path forward after that.

## Considered options

- **Auto-trigger Refresh on full Apply success** — rejected to keep Apply's existing "never auto-triggers anything" posture (ADR-0022) intact even on the happy path; the user still clicks to advance state.
- **Tag Refresh-produced Snapshots with a provenance marker** — rejected. The diff engine and roster tree already work identically regardless of how a Snapshot was produced, so a marker would be pure UI sugar bought at the cost of a new schema column and a second definition of what a Snapshot is.
- **Reuse cached/stale group membership instead of a live `list_group_members` call** — rejected; since Refresh already calls `list_group_permissions` for a target, resolving membership live keeps that target's own data internally consistent, matching what a full Run would have produced for it.
- **Discover a Project's current repo set via a project-filtered repo-list call** — rejected in favor of reusing the repo set already recorded in the source Snapshot, to keep "Refresh never discovers" an absolute rule rather than a mostly-true one.

## Consequences

- Because Snapshots carry no marker distinguishing Run from Refresh, a `FetchFailed` status on a Refresh target must mean exactly what it means for a Run — zero records for that entity, never a fallback to old copied data. A 401 aborts the whole Refresh with no new Snapshot written (mirroring Discovery failure); any other per-target failure records `FetchFailed` for just that target and Refresh continues with the rest.
- A repo added to, removed from, or moved between Projects since the last full Run will not be reflected in a Project-scope Refresh's repo set — only a subsequent full Run picks that up. This is an accepted gap, not an oversight.
- Refresh and Run now share the same "supersedes the Snapshot pending edits were staged against" trigger for clearing Pending edits (extends the rule already stated in ADR-0022 and the `Pending edit` glossary entry).
