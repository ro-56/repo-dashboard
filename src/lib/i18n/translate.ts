// Shared translate-function shape for pure logic modules (rosterRow.ts, repoCard.ts) — matches
// svelte-i18n's `$_`/MessageFormatter signature structurally, so components can pass `$_`
// straight through with no adapter, and tests can pass a plain function instead.
export type Translate = (id: string, options?: { values?: Record<string, string | number> }) => string;
