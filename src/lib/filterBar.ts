// Presentation logic for the 31px filter bar (PD-20): the All access / Changes only tab, the
// N live · N changed · N rows note, and the scope of the Expand all / Collapse all / Reset
// controls. Pure functions only — markup lives in FilterBar.svelte.

import type { ProjectNode, RepoNode, RosterTree } from "./roster";
import { isRepoOpen, repoHasChanges } from "./repoCard";
import { hasDiff } from "./rosterRow";
import type { Translate } from "./i18n/translate";

export type ViewMode = "all" | "changes";

export interface ViewCounts {
  live: number;
  changed: number;
  rows: number;
}

/** Case-insensitive substring match of `query` against every Principal's label in the repo
 * (Direct/Group/Member alike) — a blank/whitespace query always matches, so search is a no-op
 * when inactive (PD-85). */
export function matchesSearch(repo: RepoNode, query: string): boolean {
  const needle = query.trim().toLowerCase();
  if (!needle) return true;
  return repo.principals.some((entry) => entry.principal.label.toLowerCase().includes(needle));
}

/** Whether a repo is visible specifically *because of* an active search, as distinct from
 * `matchesSearch`'s "vacuously true" answer when search is inactive — a blank query must never
 * force every repo open, only an actual match against typed text does (PD-86). */
export function isSearchMatch(repo: RepoNode, query: string): boolean {
  return query.trim().length > 0 && matchesSearch(repo, query);
}

/** Repo cards visible under the current tab — a repo with zero changes disappears entirely in
 * Changes only (PD-20); a repo with no Principal matching an active search disappears entirely
 * too, ANDed on top of the view-mode predicate (PD-85). */
export function visibleRepos(project: ProjectNode, mode: ViewMode, query: string): RepoNode[] {
  const repos = mode === "all" ? project.repos : project.repos.filter(repoHasChanges);
  return repos.filter((repo) => matchesSearch(repo, query));
}

/** Rows counted under the current tab: every row in All access, only diffed rows in Changes
 * only, within whichever repos `visibleRepos` currently keeps on screen — mirrors the card-level
 * filter above so the note never disagrees with what's drawn (PD-20, PD-85). */
export function viewCounts(tree: RosterTree, mode: ViewMode, query: string): ViewCounts {
  const entries = tree.flatMap((project) =>
    visibleRepos(project, mode, query).flatMap((repo) =>
      repo.principals.filter((entry) => mode === "all" || hasDiff(entry)),
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

export function visibleRepoCount(tree: RosterTree, mode: ViewMode, query: string): number {
  return tree.reduce((sum, project) => sum + visibleRepos(project, mode, query).length, 0);
}

/** Whether every currently visible card is open — decides the bulk control's label: "the
 * control's label reflects which action it will perform" (PD-20). */
export function allVisibleOpen(
  tree: RosterTree,
  mode: ViewMode,
  treeHasChanges: boolean,
  isToggled: (repoProject: string, repo: string) => boolean,
  query: string,
): boolean {
  for (const project of tree) {
    for (const repo of visibleRepos(project, mode, query)) {
      if (!isRepoOpen(repo, treeHasChanges, isSearchMatch(repo, query), isToggled(project.repoProject, repo.repo)))
        return false;
    }
  }
  return true;
}
