---
status: accepted
---

# Vitest for agent-facing pure-TS unit tests; Playwright E2E deferred

PD-31–34 established this repo's standing frontend-testing convention: no JS test runner, manual `pnpm tauri dev` verification for every Svelte/CSS/pure-TS change (most recently reaffirmed in PD-34, which explicitly declined introducing vitest). That convention was reasoned around a human trusting the code. It doesn't hold for an AI agent modifying the UI, which needs a fast, deterministic signal it can act on mid-edit without launching the full desktop shell or relying on a screenshot.

**Decision**: add `vitest` and write unit tests for the pure-TS logic modules (`roster.ts`, `rosterRow.ts`, `repoCard.ts`, `headBar.ts`, `filterBar.ts`, `noticeStrip.ts`, `emptyState.ts`) — where the actual domain logic (diff semantics, tree building, tag derivation) lives. Coverage is added **incrementally**: a module gains tests the next time a ticket touches it, not via an upfront backfill, mirroring how `cargo test` coverage grew ticket-by-ticket on the Rust side. Tests are colocated as `*.test.ts` beside the module they cover, matching the Rust convention of test modules living alongside the code they test.

Component/E2E-level testing (Svelte rendering, visual/layout regressions) is **deferred**. `@playwright/test` is already an unused devDependency in `package.json`, but wiring it up isn't a small addition: `src/routes/dashboard/+page.ts` calls `invoke("list_snapshots")`/`invoke("get_roster_tree")` directly inside `load`, and Tauri v2's `invoke()` throws when `window.__TAURI_INTERNALS__` isn't present — so even a single visual test needs a standing IPC-mocking shim built first before it has any value. That's a fixed upfront cost with no incremental first-test payoff, unlike the vitest side, and isn't justified without a concrete visual-regression incident to point at.

## Considered Options

- **Backfill vitest coverage for all existing pure-TS modules now** — rejected: no attached ticket motivation for the untouched modules, same organic-growth profile that worked fine for `cargo test`.
- **Build the Playwright/Tauri-mock shim now, alongside vitest** — rejected for now; the cost is recorded here so it can be re-evaluated once there's a real case for it.
- **Leave the frontend fully manual (status quo per PD-31–34)** — rejected: doesn't give an agent a fast, non-screenshot-dependent signal while editing, which is the actual problem being solved.

## Consequences

- `@playwright/test` remains an unused devDependency until this decision is revisited — expected, not an oversight.
- Per-ticket "Testing Decisions" sections should start noting vitest coverage for whichever pure-TS modules a ticket touches, alongside the existing manual-verification language for Svelte/CSS changes.
- `CLAUDE.md`'s "no JS test runner configured" line and Commands section need updating when this is implemented (tracked separately from this ADR).
