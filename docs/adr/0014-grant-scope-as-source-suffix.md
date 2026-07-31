# Grant scope surfaces as a source-label suffix, not a dedicated column

Status: superseded by ADR-0020

`PrincipalEntry.scope` (`Repo` | `Project`, ADR-0012) has been part of the data model since PD-30 but was never rendered anywhere in the roster row — the new visual design (`.references/new-design/`) doesn't show it either, since it predates ADR-0012. Rather than widening the roster grid with a dedicated scope column, a Project-level grant is marked by appending `· project` to the existing source cell: `direct` → `direct · project`, `group` → `group · project`, `grp:secops` → `grp:secops · project`. Repo-level grants (the common case) render with no suffix, matching today's behavior.

This keeps the roster grid's seven tracks and their reflow order (ADR-0009) untouched — scope rides in a cell that already exists rather than claiming new horizontal space in a layout that's already tight at the 900px floor. The middle-dot separator matches the mono-punctuation style already used elsewhere on screen (e.g. the head bar's "13 days · 486 → 492 grants"). Consequence: the source column's content is no longer a fixed short enum (`direct`/`group`/`grp:<name>`) but a slightly longer, scope-qualified string — `sourceLabel()` (`src/lib/rosterRow.ts`) needs to take `scope` as an input, and its `title` tooltip should keep the unabbreviated attribute available for a truncated cell.
