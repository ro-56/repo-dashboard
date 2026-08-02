# repo-dashboard

A local-first desktop app for auditing Bitbucket Cloud repository permissions. It discovers every repo and Project in a workspace, records each fetch as an immutable timestamped Snapshot, and shows a roster + diff dashboard so permission changes — especially escalations like read → admin — are visible between any two Snapshots.

Built with [Tauri](https://tauri.app/) (Rust backend) + [SvelteKit](https://kit.svelte.dev/) (Svelte 5 frontend), storing data locally in SQLite with credentials held in the OS keychain.

## Description

Bitbucket Cloud repository access (direct grants, group grants, and project-level grants that cascade to their repos) can change outside any review process, with no built-in way to see what changed between two points in time. repo-dashboard:

- Discovers every repository and Project in a configured Bitbucket workspace (never a hardcoded list).
- Collects each Principal's (user or group) direct, group, and group-member-derived permissions, at both repo and Project scope.
- Persists every "Run" as an immutable Snapshot, and supports a targeted "Refresh" that re-fetches a narrow subset without a full rediscovery.
- Computes diffs between any two Snapshots at query time — grants, revokes, and level changes (escalation/demotion) — grouped by Project → repo → Principal.
- Lets you stage and batch-apply permission edits (level changes, removals) directly from the diff view.
- Exports a Snapshot's roster to CSV for one-off sharing/archiving (not a recurring reporting path — the dashboard is the source of truth).

For the project's domain vocabulary (Principal, Grant scope, Run vs. Refresh, Pending edit, etc.), see [`CONTEXT.md`](CONTEXT.md). Architectural decisions are recorded incrementally in [`docs/adr/`](docs/adr/).

## Installation

### Prerequisites

- [pnpm](https://pnpm.io/)
- [Rust](https://www.rust-lang.org/tools/install) (stable, via `rustup`)
- Tauri v2's platform dependencies (on Linux: `webkit2gtk`, `gtk3`, etc. — see the [Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/)). A preconfigured `.devcontainer/` is included with all of this preinstalled, along with the `taskr` CLI used to track work.

### Setup

```bash
pnpm install
```

## Usage

```bash
pnpm tauri dev        # run the full desktop app (Rust backend + webview), hot-reloading
pnpm tauri build       # produce a distributable desktop binary

pnpm dev               # vite dev server only (frontend, no Tauri shell — for quick UI iteration)
pnpm build             # build the SvelteKit app (adapter-static, SPA fallback)
pnpm check             # svelte-kit sync + svelte-check (type checking)
```

On first launch, enter a Bitbucket username + app password and workspace name via the credentials panel (stored encrypted in the OS keychain, never in plaintext), then use "Run now" to fetch the first Snapshot.

### Testing

```bash
pnpm test                      # vitest, frontend TS logic modules
cd src-tauri && cargo test     # Rust unit tests (storage, diff engine, collect, apply, refresh)
```

Svelte component rendering and end-to-end/visual testing (Playwright) are intentionally deferred — see [ADR-0017](docs/adr/0017-vitest-for-agent-feedback-defer-playwright.md). Svelte/CSS changes are currently verified manually via `pnpm tauri dev`.

## Support

This is a small-team internal tool, not a publicly supported project. Work and known issues are tracked in `taskr` (`.tickets/`, ticket prefix `PD`) — run `taskr ready` to see unblocked work.

## Roadmap

There's no formal public roadmap; upcoming work is whatever's `ready` in `taskr`. Non-goals (multi-workspace support, notifications, login/roles, CSV as a recurring export path) are listed in [`CLAUDE.md`](CLAUDE.md#non-goals-do-not-build).

## Project status

Active development — core Run/Refresh, roster/diff dashboard, permission editing, and CSV export are implemented; see recent commits and `docs/adr/` for the latest decisions.

## License

MIT, per `package.json`.
