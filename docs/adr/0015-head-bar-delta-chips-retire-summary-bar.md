# Head bar shows delta-only chips; the top-level current-state breakdown is dropped

Status: accepted

The new visual design (`.references/new-design/perm-diff.css`, `.delta-chips`) replaces `SummaryBar.svelte`'s 44px two-cluster bar — which showed both the Comparison snapshot's live admin/write/read breakdown *and* delta stats (added/revoked/changed/escalations) — with a single row of delta-only chips in the head bar, plus a terse trend line ("13 days · 486 → 492 grants"). The current-state admin/write/read breakdown that `SummaryBar` showed at the top level is dropped entirely; it is not relocated anywhere else in the head bar.

This was a deliberate trade-off, not an oversight: every repo card and project header already carries its own admin/write/read distribution bar and grant count (ADR-0008), so the workspace-wide figure was redundant with information already on screen, one scroll away. Consequence: `SummaryBar.svelte` and `summaryBar.ts`'s `currentStateStats` are retired — `leftClusterLabel`/`leftStats` and the "current" cluster have no replacement. `deltaStats` survives, reshaped into the new chip format. A future reader who wants "how many admins does this workspace have right now" without opening any card should treat that as a new feature request, not a regression to fix — the information was intentionally pushed down to card/project level.
