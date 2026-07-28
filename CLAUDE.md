# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

**repo-dashboard** — a local-first desktop app (Tauri + SvelteKit) that replaces a manual script (`.references/main.py`) for auditing Bitbucket Cloud repository permissions. It fetches, for every repo in a workspace, every user's direct and group-derived permission level, stores each run as an immutable timestamped snapshot, and shows a roster + diff dashboard so escalations (e.g. read → admin) are visible between any two runs.

The codebase is currently the stock `pnpm create tauri-app` scaffold — no app-specific frontend or backend code has been written yet. Ground truth for what to build lives in:

- `.references/PRD.md` — full product spec (functional requirements, data model, non-goals). Read this before implementing any feature.
- `.references/project-ref.md` — IA rationale (why "grouped by repo" won over other layouts).
- `.references/main.py` — the Python script whose data-collection logic must be ported (repo discovery, direct/group permission fetch, per-repo 404 handling).
- `.references/dashboard_designs.html` — finalized visual design with prototyped states (`1a`–`1i`) for the roster/diff dashboard.

Work is tracked in taskr (`.tickets/`, prefix `PD`). Run `taskr ready` to see unblocked work and `taskr show <id>` before starting any ticket.

## Non-goals (do not build)

- Multi-workspace or multi-provider support — single hardcoded-but-configurable Bitbucket Cloud workspace.
- Any notification/alerting channel — purely pull-based.
- Excel/CSV export — the dashboard fully replaces the old Excel report.
- Login, roles, or any multi-user auth model.
- Group membership as a diffable entity — group grants are flattened into per-user rows at collection time; the originating group is kept only as metadata (`access_type: group:<name>`), never diffed as its own dimension.

## Commands

```bash
pnpm install          # install JS deps
pnpm dev              # vite dev server only (frontend, no Tauri shell)
pnpm build            # build SvelteKit app (adapter-static, SPA fallback)
pnpm check            # svelte-kit sync + svelte-check (type checking)
pnpm check:watch      # same, in watch mode

pnpm tauri dev        # run the full desktop app (Rust backend + webview), hot-reloading
pnpm tauri build       # produce a distributable desktop binary

cd src-tauri && cargo test     # run Rust unit tests (backend logic: storage, diff engine, fetch)
cd src-tauri && cargo check    # fast Rust compile check
```

There is no JS test runner configured yet. Backend logic (storage, diff engine, Bitbucket fetch) should be tested with `cargo test` in `src-tauri`, per the acceptance criteria in `PD-2`, `PD-5`, and `PD-6`.

## Architecture

- **Frontend**: SvelteKit (Svelte 5) in SPA mode via `@sveltejs/adapter-static` — there is no SvelteKit server; all routing/rendering happens client-side inside the Tauri webview. Routes live under `src/routes/`.
- **Backend**: Rust crate `repo_dashboard_lib` in `src-tauri/src/`, exposing Tauri commands (see `lib.rs`) that the frontend invokes via `@tauri-apps/api`. This is where the Bitbucket HTTP client, SQLite storage, diff engine, and credential handling belong — not in the frontend.
- **Data flow**: "Run now" (frontend) → Tauri command → Rust fetches Bitbucket API (repo discovery is dynamic, never a hardcoded repo list) → normalizes direct + group permissions into flat `PermissionRecord` rows → persists one new immutable `Snapshot` in SQLite. Diffs between two snapshots are computed at query time in Rust, not stored, keyed on `(repo, username)`.
- **Data model** (PRD §7):
  ```
  Snapshot: id, run_at
  PermissionRecord: snapshot_id, repo_project, repo, username, display_name,
                    access_type ("direct" | "group:<name>"), permission (read|write|admin)
  ```
- **Credentials**: entered via the app UI, stored encrypted at rest through an OS-level mechanism (Tauri keychain plugin / system keyring) — never a plaintext file, never a hardcoded fallback like `main.py`'s current (live, needs rotating) credential.
- **Diff semantics**: ordering is `read < write < admin`. Grant = in B not A; Revoke = in A not B; Level change = in both with different permission (escalation if up, demotion if down); a repo missing entirely from one run is flagged at the repo level (struck-through in the UI), not silently dropped.

## Environment

Dev container (`.devcontainer/`) provides pnpm, Rust (stable, via rustup), Tauri v2 Linux system deps (webkit2gtk, gtk3, etc.), and the `taskr` CLI preinstalled.
