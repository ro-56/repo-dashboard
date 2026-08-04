// Presentation logic for the tree-top "N principals hidden this session" banner (CONTEXT.md's
// Excluded principal, ADR-0032, PD-92) — the sole surface for viewing/removing exclusions added
// via RosterRow's "Hide" link (PD-91). Pure functions only — markup lives in
// ExcludedBanner.svelte.

import type { RosterTree } from "./roster";
import type { Translate } from "./i18n/translate";

export interface ExcludedPrincipalView {
  id: string;
  label: string;
}

/** One entry per excluded id, labelled from the first row anywhere in the tree carrying that
 * `principal.id` — an own-id match, same identity `isExcluded` (rosterRow.ts) matches on, so a
 * hidden Group's own row supplies the Group's label rather than one of the Member rows it
 * cascades to. Falls back to the id itself when the tree carries no matching row at all, so an
 * exclusion is never silently dropped from the manage list. Order follows `excludedIds`'
 * iteration order (a `Set` preserves insertion order), i.e. the order Principals were hidden in. */
export function excludedPrincipals(tree: RosterTree, excludedIds: ReadonlySet<string>): ExcludedPrincipalView[] {
  const labelById = new Map<string, string>();
  for (const project of tree) {
    for (const repo of project.repos) {
      for (const entry of repo.principals) {
        if (!labelById.has(entry.principal.id)) labelById.set(entry.principal.id, entry.principal.label);
      }
    }
  }
  return [...excludedIds].map((id) => ({ id, label: labelById.get(id) ?? id }));
}

export function excludedBannerCount(t: Translate, excludedIds: ReadonlySet<string>): string {
  return t("excludedBanner.count", { values: { count: excludedIds.size } });
}
