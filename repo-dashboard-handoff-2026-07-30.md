# Handoff: repo-dashboard "data not showing after Run" bug investigation

**Date:** 2026-07-30
**Repo:** /workspace (repo-dashboard, Tauri + SvelteKit)
**Next session focus (per user):** continue investigating the error — the fixes below are
implemented and unit-tested but **not yet confirmed against the real running app**.

## User-reported symptoms (two rounds)

**Round 1:** Since commit `c78cdd4` (PD-28, grant scope model), after clicking "Get all data"
the dashboard kept showing "No runs recorded yet"; after closing and reopening the app, the
screen went fully blank.

**Round 2 (after Round 1's fix was in place):** On a completely fresh SQLite db (user deleted
the old file, reopened the app, ran "Get all data"), the fetch finished but the dashboard and
the side drawer's snapshot list both showed nothing — **despite the user confirming via direct
inspection that a snapshot row actually exists in the database.**

## Root causes found and fixed (both uncommitted in the working tree)

### Bug 1 — missing schema migration for the `scope` column (Round 1)

`src-tauri/src/storage.rs::init_schema` used `CREATE TABLE IF NOT EXISTS`, which is a no-op on
any pre-existing `permission_records` table from before PD-28 — such a table never gains the
`scope` column PD-28 added. That breaks both `save_snapshot` (INSERT references `scope`) and
`load_snapshot` (SELECT references `scope`) against any pre-existing local dev database.

**Fix:** `init_schema` now detects a missing `scope` column and runs `ALTER TABLE
permission_records ADD COLUMN scope TEXT NOT NULL DEFAULT 'repo'` to backfill it. Covered by
`storage::legacy_schema_upgrade_tests` (backfill + idempotency + a load-after-migration
regression test).

This bug required a **pre-existing** (pre-PD28-shaped) local database to trigger — it does not
explain Round 2, which started from a freshly deleted database.

### Bug 2 — non-atomic Run persistence (Round 2, the "data in DB but screen empty" bug)

`collect_and_store` (`src-tauri/src/collect.rs`) used to write a Run's data in two separate
steps:
1. `storage::save_snapshot` — its own transaction, committing `snapshots` +
   `permission_records` + `repo_fetch_statuses` + `group_membership_statuses`.
2. `storage::save_project_fetch_statuses` — a **separate**, later, non-transactional call.

If step 2 failed for any reason, `collect_and_store` returned `Err` even though step 1 had
already durably committed a real Snapshot. `RunPanel.svelte`'s `runNow()` sees the `Err`, shows
an error, and — critically — never calls `goto(..., { invalidateAll: true })`, so the dashboard
never re-fetches `list_snapshots`. The UI is stuck showing stale (empty) data, even though the
snapshot is genuinely in the database and would show up on the next app restart.

Confirmed with a reproduction test that forced the second write to fail (dropping the
`project_fetch_statuses` table between `init_schema` and the run) and observed a committed
snapshot despite an `Err` result.

**Fix:** folded `project_statuses` into `save_snapshot`'s single transaction as a new parameter
(mirroring how `group_membership_statuses` already worked), so the whole Run is now atomic —
everything commits or nothing does. Removed the now-dead `save_project_fetch_statuses` write
function (kept `load_project_fetch_statuses`, a read). Updated ~50 call sites across
`storage.rs`/`roster.rs`/`collect.rs` (mostly mechanical — added a 5th `&[]`/status-list arg).
The old repro test was flipped into a regression test asserting the failure now rolls back the
*whole* snapshot instead of leaving an orphaned one.

## Current repo state

- `cargo test` in `src-tauri`: **103 passed, 0 failed**. `cargo check`: clean.
- Everything is **uncommitted**. `git status` shows modified: `.tickets/config.json`,
  `src-tauri/src/{collect,roster,storage}.rs`.
- I proposed committing the two fixes but the user ran `/handoff` before responding — treat the
  commit as **not yet authorized**.
- The repo's rustfmt is already not clean at baseline across many pre-existing files (verified
  by stashing and re-running `cargo fmt --check`) — this is not something introduced by this
  session's changes, and not worth touching.

## Untracked files present but NOT part of this bug-fix work

`.tickets/PD-31.md`, `.tickets/PD-32.md`, `.tickets/PD-33.md`,
`docs/adr/0013-run-progress-via-injected-callback.md` — these describe an unrelated feature
("show live fetch progress during Run now") that appears to have been planned in a separate,
earlier session (not this one). Don't fold them into the bug-fix commit. Worth asking the user
whether these are intentional in-progress work to keep, or should be reviewed/discarded — I did
not touch or investigate them beyond reading their contents.

## What's NOT yet done / open risk

- **Neither fix has been exercised against the real running app.** This environment has no real
  Bitbucket credentials, so "Get all data" was never driven end-to-end through
  `pnpm tauri dev` — all verification so far is `cargo test` with `FakeBitbucketClient`. The
  user should test both scenarios for real:
  1. A pre-existing (pre-PD28) local db → run "Get all data" → confirm data now shows.
  2. A freshly deleted db → run "Get all data" → confirm data now shows immediately (no
     restart needed).
- If either symptom recurs after these fixes, do **not** assume it's the same root cause —
  re-derive from the actual current behavior. In particular, I never got confirmation on
  whether the user saw an error message in the Run panel during Round 2 (RunPanel.svelte shows
  `error` text right in the drawer on failure) — worth asking directly this time, since it would
  confirm or refute the Bug 2 diagnosis.
- No SvelteKit `+error.svelte` boundary exists anywhere in `src/routes/` — if `+page.ts`'s
  `load()` ever throws (e.g. `get_roster_tree` erroring), the user sees SvelteKit's bare default
  error page, which reads as a "blank screen." That was relevant to Round 1's second symptom.
  Worth considering whether to add a proper error boundary regardless, so future backend errors
  degrade gracefully instead of looking like the app is broken.

## Suggested skills for the next session

- **taskr** — this repo tracks work as tickets in `.tickets/` (prefix `PD`); if the live-app
  verification above surfaces a genuinely new/different bug, or once these fixes are confirmed,
  consider filing/closing tickets through it rather than leaving the fix undocumented.
- **code-review** — before committing, run this against the diff (`collect.rs`, `roster.rs`,
  `storage.rs`) since it's a fairly wide-reaching mechanical change (~50 call sites) that's easy
  to eyeball-miss a mistake in.
- **run** — to actually launch the Tauri app and drive "Get all data" live, which is the missing
  verification step called out above.
- **tdd** — this codebase's existing convention (see `collect.rs`/`storage.rs`/`roster.rs` test
  modules) is fake-client-driven, `cargo test`-first; keep following that pattern for any further
  backend changes.
