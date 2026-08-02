# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

**repo-dashboard** — a local-first desktop app (Tauri + SvelteKit) for auditing Bitbucket Cloud repository permissions. It fetches, for every repo (and Project) in a workspace, every Principal's (user or group) direct/group/member-derived permission, stores each Run as an immutable timestamped Snapshot, and shows a roster + diff dashboard so escalations (e.g. read → admin) are visible between any two Snapshots.

Ground truth for what to build lives in:

- **`CONTEXT.md`** — the domain glossary (Principal, Grant scope, Run vs. Refresh, Pending edit, etc.) — read this before touching diff/roster/collect/apply logic, since the vocabulary is precise and load-bearing.
- **`docs/adr/`** — one Markdown file per architectural decision, numbered in order (`0001`...`0026`). `CONTEXT.md` and the code comments cite these directly (e.g. "ADR-0021, `docs/adr/0021-permission-editing-scope-boundaries.md`"); check the relevant ADR before changing behavior it governs rather than re-deriving the rationale from scratch.

Work is tracked in taskr (`.tickets/`, prefix `PD`). Run `taskr ready` to see unblocked work and `taskr show <id>` before starting any ticket.

## Non-goals (do not build)

- Multi-workspace or multi-provider support — single hardcoded-but-configurable Bitbucket Cloud workspace.
- Any notification/alerting channel — purely pull-based.
- Excel/CSV export as a recurring reporting path — the dashboard remains the source of truth. (Carve-out: a one-off CSV Roster export is in scope — see ADR-0026.)
- Login, roles, or any multi-user auth model.
- Group membership as a diffable entity in its own right — a group's own grant (`AccessType::Group`) is diffable, but its *membership list* is not; membership changes surface only as the resulting Member grants appearing/disappearing.

## Commands

```bash
pnpm install          # install JS deps
pnpm dev              # vite dev server only (frontend, no Tauri shell)
pnpm build            # build SvelteKit app (adapter-static, SPA fallback)
pnpm check            # svelte-kit sync + svelte-check (type checking)
pnpm check:watch      # same, in watch mode
pnpm test             # vitest, run once and exit (CI-style)
pnpm test:watch       # vitest, watch mode
pnpm vitest run src/lib/roster.test.ts   # run a single frontend test file

pnpm tauri dev        # run the full desktop app (Rust backend + webview), hot-reloading
pnpm tauri build      # produce a distributable desktop binary

cd src-tauri && cargo test                     # run all Rust unit tests
cd src-tauri && cargo test admin_light          # run tests matching a substring (across files)
cd src-tauri && cargo test --lib diff::tests::  # run tests in one module
cd src-tauri && cargo check                     # fast Rust compile check
```

Backend logic (storage, diff engine, fetch/collect, apply, refresh) is tested with `cargo test` in `src-tauri` — every `src-tauri/src/*.rs` file carries its own `#[cfg(test)] mod tests` block; there are no separate integration-test files. Frontend pure-TS logic modules under `src/lib/` (`roster.ts`, `rosterRow.ts`, `repoCard.ts`, `headBar.ts`, `filterBar.ts`, `emptyState.ts`, `rosterExport.ts`, `pendingEdits.ts`, `theme.ts`, ...) get colocated `*.test.ts` vitest files incrementally — a module gains one the next time a ticket touches it, not via upfront backfill (ADR-0017, `docs/adr/0017-vitest-for-agent-feedback-defer-playwright.md`). Svelte component rendering and E2E/visual testing (Playwright) remain deferred per the same ADR — `@playwright/test` is an intentionally-unused devDependency until that's revisited. Svelte/CSS changes are still verified manually via `pnpm tauri dev`.

## Architecture

- **Frontend**: SvelteKit (Svelte 5) in SPA mode via `@sveltejs/adapter-static` — there is no SvelteKit server; all routing/rendering happens client-side inside the Tauri webview. Routes live under `src/routes/` (`+layout.svelte`, the entry `+page.svelte`, and `dashboard/+page.svelte` for the main roster view). Pure logic is factored out of components into colocated `src/lib/*.ts` modules (e.g. `roster.ts`, `rosterExport.ts`, `pendingEdits.ts`) so it's independently unit-testable without rendering Svelte; UI components live in `src/lib/components/` (`RepoCard`, `RosterRow`, `ProjectSection`, `HeadBar`, `FilterBar`, `SideDrawer`, `RunPanel`, `SnapshotsPanel`, `CredentialsPanel`, `ConfirmApplyDialog`, `AppearancePanel`, `CountBadges`, `EmptyState`).
- **Backend**: Rust crate `repo_dashboard_lib` in `src-tauri/src/`, exposing Tauri commands from `lib.rs` that the frontend invokes via `@tauri-apps/api`:
  - `set_credentials` / `get_credentials` — OS-keychain-backed credential storage (never returns the app password to the frontend).
  - `run_now` — full workspace discovery + fetch, produces one new Snapshot (`collect.rs`).
  - `refresh_snapshot` — copies a source Snapshot forward and re-fetches only a targeted set of repos/Projects, no discovery (`refresh.rs`, ADR-0025).
  - `list_snapshots` — Snapshot picker data.
  - `get_roster_tree` — the project → repo → Principal tree for a Run pair, diffed at query time (`roster.rs`).
  - `delete_snapshot` — Snapshots are deletable (ADR-0011).
  - `apply_pending_edits` — batch-applies staged Level-change/Remove edits against Bitbucket (`apply.rs`, ADR-0022).
  - Module map: `client.rs` (Bitbucket HTTP client), `collect.rs` (Run orchestration), `normalize.rs` (raw API → `PermissionRecord`), `storage.rs` (SQLite persistence), `diff.rs` (Grant/Revoke/Level-change classification), `roster.rs` (tree assembly), `credentials.rs` (keyring), `apply.rs` (permission edits), `refresh.rs`, `model.rs` (core types).
- **Data flow**: "Run now" (frontend) → `run_now` → Rust discovers every repo/Project in the workspace (never a hardcoded list) → fetches direct/group grants per repo and Project → `normalize.rs` flattens them into `PermissionRecord` rows (resolving group membership into `Member` grants where possible) → `collect.rs` persists one new immutable `Snapshot` in SQLite, emitting `run-progress` events as it goes. "Refresh" (`refresh_snapshot`) does the same fetch/normalize/persist steps but skips discovery, re-fetching only an explicitly addressed subset. Diffs between two Snapshots are computed at query time in `roster.rs`/`diff.rs`, never stored, keyed on `(repo, Principal, access_type, scope)`.
- **Core types** (`src-tauri/src/model.rs`) — read `CONTEXT.md` for the full vocabulary; summary:
  - `Principal { id, label }` — a user (`id` = Bitbucket `account_id`) or a group (`id` = slug). Identity is always the stable id, never the display label.
  - `AccessType` — `Direct` | `Group` | `Member(group_id)`. A group's own grant (`Group`) is a first-class, independently diffable record (ADR-0002); a `Member` grant is derived from it but tracked separately, never merged (ADR-0001).
  - `Permission` — `Read < Write < CreateRepo < Admin` (derived `Ord`). `CreateRepo` is Project-scope only in practice; `admin_light()` collapses it onto `Admin` for diff/count computation while display code shows it distinctly (ADR-0024).
  - `GrantScope` — `Repo` | `Project`, orthogonal to `AccessType` (ADR-0012): a Project-level grant cascades to every repo it owns, and the same Principal can hold independent Repo- and Project-level records.
  - `PermissionRecord { repo_project, repo, principal, access_type, scope, permission }`.
  - `RepoFetchStatus` / `ProjectFetchStatus` — per-repo / per-Project fetch outcome (`Ok` | `FetchFailed`) for a Snapshot; absence of a row means "not discovered this run", distinct from `FetchFailed`.
  - `GroupMembershipStatus` — whether a group's member list was resolvable, tracked separately from the group's own grant.
- **Credentials**: entered via the app UI, stored encrypted at rest through an OS-level keyring (`keyring-core` + platform-native backend: Keychain on macOS, Windows credential store, secret-service on Linux) — never a plaintext file, never a hardcoded fallback.
- **Diff semantics**: ordering is `read < write < create-repo < admin` (`create-repo` collapses to admin-equivalent for diff purposes only, ADR-0024). Grant = record in the Comparison Snapshot with no match in the Baseline; Revoke = the reverse; Level change = same record (same Principal + repo + access_type + scope) in both with a different permission (Escalation if up, Demotion if down). A repo/Project missing entirely from one Snapshot is flagged at that level (Repo absence/arrival), not silently dropped as per-row diffs.
- **Pending edits**: Level-change/Remove edits against a Direct or Group grant can be staged locally in UI state and applied as a batch via `apply_pending_edits` — never sent automatically, only valid while the Comparison Snapshot is still the latest one, and discarded (not carried forward) once superseded by a new Run or Refresh (ADR-0022).
