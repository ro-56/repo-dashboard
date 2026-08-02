// Presentation logic for the 31px filter bar (PD-20): the All access / Changes only tab, the
// N live · N changed · N rows note, and the scope of the Expand all / Collapse all / Reset
// controls. Pure functions only — markup lives in FilterBar.svelte.

import type { PrincipalEntry, ProjectNode, RepoNode, RosterTree } from "./roster";
import { isRepoOpen, repoHasChanges } from "./repoCard";
import { hasDiff } from "./rosterRow";
import type { Translate } from "./i18n/translate";

export type ViewMode = "all" | "changes";

export interface ViewCounts {
  live: number;
  changed: number;
  rows: number;
}

function allEntries(tree: RosterTree): PrincipalEntry[] {
  return tree.flatMap((project) => project.repos.flatMap((repo) => repo.principals));
}

/** Rows counted under the current tab: every row in All access, only diffed rows in Changes
 * only — mirrors the card-level filter below so the note never disagrees with what's drawn. */
export function viewCounts(tree: RosterTree, mode: ViewMode): ViewCounts {
  const entries = allEntries(tree).filter((entry) => mode === "all" || hasDiff(entry));
  return {
    live: entries.filter((entry) => entry.diffStatus.status !== "Revoke").length,
    changed: entries.filter(hasDiff).length,
    rows: entries.length,
  };
}

export function countNote(t: Translate, counts: ViewCounts): string {
  return t("filterBar.countNote", { values: { live: counts.live, changed: counts.changed, rows: counts.rows } });
}

/** Repo cards visible under the current tab — a repo with zero changes disappears entirely
 * in Changes only (PD-20). */
export function visibleRepos(project: ProjectNode, mode: ViewMode): RepoNode[] {
  return mode === "all" ? project.repos : project.repos.filter(repoHasChanges);
}

export function visibleRepoCount(tree: RosterTree, mode: ViewMode): number {
  return tree.reduce((sum, project) => sum + visibleRepos(project, mode).length, 0);
}

/** Whether every currently visible card is open — decides the bulk control's label: "the
 * control's label reflects which action it will perform" (PD-20). */
export function allVisibleOpen(
  tree: RosterTree,
  mode: ViewMode,
  treeHasChanges: boolean,
  isToggled: (repoProject: string, repo: string) => boolean,
): boolean {
  for (const project of tree) {
    for (const repo of visibleRepos(project, mode)) {
      if (!isRepoOpen(repo, treeHasChanges, isToggled(project.repoProject, repo.repo))) return false;
    }
  }
  return true;
}
