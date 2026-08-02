# Discrepancy report — repo-dashboard user guide

Internal notes on where documentation and code disagreed, or where behavior was inferred from
code with no doc source. Not shown to end users.

## Undocumented but real (added to guide from code alone)

- **Permission editing (staged edits + Apply) and Refresh** — README/CLAUDE.md mention "Lets you
  stage and batch-apply permission edits" only in passing (one bullet), with no user-facing
  walkthrough anywhere. The actual UI (row `⋯` menu, cascade-count warnings for group removals,
  per-item apply results, retry-on-failure, "Refresh affected") is entirely undocumented outside
  ADRs 0021/0022/0025. Inferred from `RosterRow.svelte`, `ConfirmApplyDialog.svelte`,
  `pendingEdits.ts`, `apply.rs`, and `dashboard/+page.svelte`.
- **Editing is gated on Comparison being the latest snapshot** — not mentioned anywhere for end
  users; only in ADR-0022/ADR-0021 code comments. Documented in the guide's "Changing or removing
  access" callout.
- **Credential scope for editing is checked lazily** (ADR-0023) — nothing tells the user up front
  that a read-only app password will silently fail only at Apply time. Documented as a callout
  under "Getting started" and in the FAQ.
- **CSV export column schema and filename convention** — not documented anywhere outside
  `rosterExport.ts`. Filled in from the source (`repo_project, repo, username, display_name,
  access_type, scope, permission, run_at`; filename `roster_<workspace>_<timestamp>.csv`).
- **Snapshot numbering scheme** ("run 1" = the first run ever, oldest-first numbering despite the
  API returning newest-first) — inferred from `headBar.ts`'s `snapshotSeqs`.
- **Default expand/collapse behavior for repo cards** (changed repos open automatically; if
  nothing changed anywhere, everything opens instead of collapsing) — inferred from
  `repoCard.ts`'s `defaultOpen`/`isRepoOpen`, referenced only in ADR-0009/PD-16 ticket comments.
- **"Expand all"/"Collapse all" label reflects the pending action, scoped to the current tab's
  visible cards** — inferred from `filterBar.ts`'s `allVisibleOpen`.
- **`members unresolved` behavior** (group grant recorded, but zero Member rows shown when
  membership can't be fetched) — present in `CONTEXT.md` domain glossary but never surfaced in
  README/user-facing text. Added to guide.
- **`create-repo` permission level and its admin-equivalence for diffing** — documented only in
  `CONTEXT.md`/ADR-0024, not in README. Added a callout.
- **Same-snapshot comparison is valid** (Baseline == Comparison shows a plain, diff-free roster,
  not an error) — from `CONTEXT.md`'s "Run pair" entry and `headBar.ts`'s `spanNote`; not surfaced
  to users anywhere else.
- **Discovery failure vs. fetch failure vs. Project fetch failure** are three distinct,
  differently-scoped failure states with different consequences (a Discovery failure blocks the
  whole Run and saves nothing; a Fetch/Project fetch failure is scoped and the Run continues) —
  this distinction exists only in `CONTEXT.md` and is not explained to end users anywhere.
  Documented in the "Taking a snapshot" section and FAQ.
- **Apply failure reasons** (`Unauthorized` → "credential lacks the required scope",
  `RateLimited` → "rate limited — retry", `Other` → raw message) — only in
  `pendingEdits.ts`/`apply.rs`, no user doc. Documented as a table in the guide.

## Docs vs. code — consistent (verified, no discrepancy)

- README's feature bullets (discovery, direct/group/member-derived permissions, Run vs. Refresh,
  diff computation, staged batch edits, CSV export) all matched current code behavior on
  inspection — no stale claims found.
- CLAUDE.md's non-goals list (no multi-workspace, no notifications, no login/roles, CSV as a
  one-off only) matches the actual UI surface — there is exactly one workspace field, no
  notification/alert code path, and no login screen (just Bitbucket credential storage).
- ADR-0026's CSV carve-out claim ("flat, unfiltered, diff-free roster of the Comparison snapshot
  alone") matches `rosterExport.ts` and the `handleExport` implementation in
  `dashboard/+page.svelte` exactly.

## Genuinely unclear / not verifiable from code alone

- **Exact required Bitbucket app-password scopes** for read vs. write operations are referenced
  only generically ("repository:admin/project:admin" for writes, "read:repository:bitbucket,
  read:workspace:bitbucket" for reads) in ADR-0023's prose — the actual Bitbucket API scope names
  in use aren't independently verified against Bitbucket's current API docs (external system, not
  inspectable from this repo). The guide describes this qualitatively ("admin rights on
  repositories and projects") rather than naming exact scope strings, to avoid asserting something
  unverified.
- **Rate-limit backoff/retry behavior** — the code surfaces a `RateLimited` error per failed apply
  item but there's no visible automatic retry logic; whether Bitbucket's actual rate-limit windows
  make "just click Apply again shortly" reliable advice depends on Bitbucket's live throttling
  behavior, which can't be confirmed from static code alone.
