// Presentation logic for the 31px filter bar: the All access / Changes only tab, the
// N live · N changed · N rows note, and the scope of the Expand all / Collapse all / Reset
// controls. Pure functions only — markup lives in FilterBar.svelte.

import type { PrincipalEntry, ProjectNode, RepoNode, RosterTree } from "./roster";
import { isRepoOpen } from "./repoCard";
import { hasDiff, isExcluded, matchesSearchQuery } from "./rosterRow";
import type { Translate } from "./i18n/translate";

export type ViewMode = "all" | "changes";

export interface ViewCounts {
  live: number;
  changed: number;
  rows: number;
}

/** Case-insensitive substring match of `query` against every Principal's label in the repo
 * (Direct/Group/Member alike) — a blank/whitespace query always matches, so search is a no-op
 * when inactive. */
export function matchesSearch(repo: RepoNode, query: string): boolean {
  if (!query.trim()) return true;
  return repo.principals.some((entry) => matchesSearchQuery(entry, query));
}

/** Whether a repo is visible specifically *because of* an active search, as distinct from
 * `matchesSearch`'s "vacuously true" answer when search is inactive — a blank query must never
 * force every repo open, only an actual match against typed text does. */
export function isSearchMatch(repo: RepoNode, query: string): boolean {
  return query.trim().length > 0 && matchesSearch(repo, query);
}

/** Whether a single row renders under the current tab + search + exclusion filter: Changes only
 * additionally requires a diff, an active search additionally requires *this row's own* Principal
 * to match, and an Excluded principal (CONTEXT.md, ADR-0032) is never shown regardless of the
 * other two — a repo kept visible by a match in one row draws only that row (and, in Changes only,
 * only diffed, non-excluded rows), never the whole roster unfiltered (an earlier version showed
 * full context — every row once a repo matched — but that read as a bug once search shipped, so
 * this narrows to matching rows only). Shared by the repo-visibility
 * check below, the count note, and RepoCard.svelte's own row filter so none of the three can
 * disagree about which rows are on screen. */
export function isEntryVisible(entry: PrincipalEntry, mode: ViewMode, query: string, excludedIds: ReadonlySet<string>): boolean {
  return (mode === "all" || hasDiff(entry)) && matchesSearchQuery(entry, query) && !isExcluded(entry, excludedIds);
}

/** Repo cards visible under the current tab — a repo disappears entirely once none of its rows
 * pass `isEntryVisible`: zero changes in Changes only, zero Principals matching an active search,
 * every remaining row excluded, or any combination thereof. */
export function visibleRepos(project: ProjectNode, mode: ViewMode, query: string, excludedIds: ReadonlySet<string>): RepoNode[] {
  return project.repos.filter((repo) => repo.principals.some((entry) => isEntryVisible(entry, mode, query, excludedIds)));
}

/** Rows counted under the current tab: exactly the rows `isEntryVisible` keeps, within whichever
 * repos `visibleRepos` currently keeps on screen — mirrors the card-level filter above so the
 * note never disagrees with what's drawn. */
export function viewCounts(tree: RosterTree, mode: ViewMode, query: string, excludedIds: ReadonlySet<string>): ViewCounts {
  const entries = tree.flatMap((project) =>
    visibleRepos(project, mode, query, excludedIds).flatMap((repo) =>
      repo.principals.filter((entry) => isEntryVisible(entry, mode, query, excludedIds)),
    ),
  );
  return {
    live: entries.filter((entry) => entry.diffStatus.status !== "Revoke").length,
    changed: entries.filter(hasDiff).length,
    rows: entries.length,
  };
}

export function countNote(t: Translate, counts: ViewCounts): string {
  return t("filterBar.countNote", { values: { live: counts.live, changed: counts.changed, rows: counts.rows } });
}

export function visibleRepoCount(tree: RosterTree, mode: ViewMode, query: string, excludedIds: ReadonlySet<string>): number {
  return tree.reduce((sum, project) => sum + visibleRepos(project, mode, query, excludedIds).length, 0);
}

/** Whether every currently visible card is open — decides the bulk control's label: "the
 * control's label reflects which action it will perform". */
export function allVisibleOpen(
  tree: RosterTree,
  mode: ViewMode,
  treeHasChanges: boolean,
  isToggled: (repoProject: string, repo: string) => boolean,
  query: string,
  excludedIds: ReadonlySet<string>,
): boolean {
  for (const project of tree) {
    for (const repo of visibleRepos(project, mode, query, excludedIds)) {
      if (!isRepoOpen(repo, treeHasChanges, isSearchMatch(repo, query), isToggled(project.repoProject, repo.repo)))
        return false;
    }
  }
  return true;
}
