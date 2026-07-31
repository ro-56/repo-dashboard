# Grant scope surfaces as a source-label prefix glyph, not a text suffix

Status: accepted

Supersedes: ADR-0014

ADR-0014 marked a Project-level grant by appending `" · project"` to the `.source` cell's base label: `direct` → `direct · project`, `grp:secops` → `grp:secops · project`. In practice this routinely truncates — `.source` is a fixed 104px track that ADR-0009 requires to never shed under responsive reflow, and a group-derived label combined with the suffix can reach ~21 characters. The `title` tooltip that was meant to back up a truncated cell just echoed the same already-cut-off string back, so Grant scope — the one thing the suffix existed to preserve — was frequently the exact attribute hidden from a workspace admin scanning the roster.

The suffix is replaced with a prefix glyph, `↳` (U+21B3, DOWNWARDS ARROW WITH TAIL): `↳ direct`, `↳ grp:secops`. Repo-scoped grants (the common case) remain unmarked, matching today's behavior. `sourceLabel()` (`src/lib/rosterRow.ts`) now prepends `"↳ "` instead of appending `" · project"`; a new `sourceTooltip()` function takes over the `title` attribute, returning a short explanatory string ("Project-level grant") for Project-scoped rows instead of echoing the visible label, and continuing to echo the visible label for Repo-scoped rows (nothing new to explain).

U+21B3 was chosen over the visually similar `⤷` (U+2934, "RIGHT ARROW CURVING DOWN") because it sits in the core Arrows block (U+2190–21FF) rather than Miscellaneous Symbols and Arrows, giving it broader font coverage — it's more likely to be covered by `IBM Plex Mono` (`--font-mono`) or a visually consistent fallback than a glyph from the miscellaneous block, avoiding a silent font-fallback mismatch next to monospace text.

The glyph carries no independent hue: it inherits `.source`'s existing `color: var(--ink-3)`, per ADR-0005's rule that any state which is neither a permission level nor a diff outcome must express itself through tags or gutters (or, as here, existing ink), never a color of its own.

Scope stays in `.source` rather than moving to `.tags`: it's a structural/identity fact — part of the same composite grant-source key as `AccessType` (ADR-0012) — not an exceptional per-row state on the level of an escalation or an unresolved group. Because a single Project-level grant flattens into one row per cascaded repo (ADR-0012), Project-scoped rows can be the *majority* in a Project-heavy workspace; putting the marker in `.tags` would dilute that column's "something unusual here" signal.

This keeps ADR-0014's structural decisions intact — scope still rides in the existing `.source` cell rather than claiming new horizontal space, and Repo-scope is still the unmarked default. Only the encoding (glyph vs. text) changes, in response to the truncation problem observed in practice.
