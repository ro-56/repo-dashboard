# Dual light/dark theme tokens adopted; toggle lives in the side drawer

Status: accepted

The new visual design ships a complete second token set (`:root[data-theme="dark"]` in `perm-diff.css`, demonstrated via separate `perm-diff-ledger-light.html`/`-dark.html` bundles). The app had no theming before this — `tokens.css` defined exactly one palette. We're adopting both token sets and a real user-facing toggle, not just porting the light palette and keeping dark as unused reference CSS.

The toggle lives inside the side drawer (ADR-0010) alongside credentials/Run/snapshot management, rather than as its own icon in the head bar — the drawer is already the home for cross-cutting app-level controls, and the head bar's icon budget is spent on the run-pair selector and delta chips. Choice persists across launches (`localStorage`), defaulting to `prefers-color-scheme` on first launch. Consequence: every component's scoped styles must resolve colors through `tokens.css` custom properties with no hardcoded hex (already the rule per ADR-0005, now load-bearing for correctness rather than just consistency) — a hardcoded color is now a dark-mode bug, not just a style nit.
