// Presentation logic for repo cards and project sections (ADR-0008, ADR-0009). Pure functions
// only — markup and colour live in RepoCard.svelte / ProjectSection.svelte's scoped styles.

import type { PrincipalEntry, ProjectNode, RepoNode, RosterTree } from "./roster";

export interface CountBadge {
  kind: "added" | "removed" | "modified" | "esc" | "none";
  label: string;
}

interface Breakdown {
  added: number;
  removed: number;
  modified: number;
  esc: number;
}

function emptyBreakdown(): Breakdown {
  return { added: 0, removed: 0, modified: 0, esc: 0 };
}

function addBreakdown(a: Breakdown, b: Breakdown): Breakdown {
  return {
    added: a.added + b.added,
    removed: a.removed + b.removed,
    modified: a.modified + b.modified,
    esc: a.esc + b.esc,
  };
}

function breakdownOf(principals: PrincipalEntry[]): Breakdown {
  const breakdown = emptyBreakdown();
  for (const p of principals) {
    switch (p.diffStatus.status) {
      case "Grant":
        breakdown.added++;
        break;
      case "Revoke":
        breakdown.removed++;
        break;
      case "LevelChange":
        breakdown.modified++;
        if (p.diffStatus.kind === "Escalation") breakdown.esc++;
        break;
    }
  }
  return breakdown;
}

function isHot(breakdown: Breakdown): boolean {
  return breakdown.added > 0 || breakdown.removed > 0 || breakdown.modified > 0;
}

/** Card and project counts: `+n`, `−n`, `~n` plus `↑n esc` when escalations exist, or `no
 * change` when nothing did — never a blank space or a zero (ADR-0008: labelled by grants). */
export function countBadges(breakdown: Breakdown): CountBadge[] {
  const badges: CountBadge[] = [];
  if (breakdown.added) badges.push({ kind: "added", label: `+${breakdown.added}` });
  if (breakdown.removed) badges.push({ kind: "removed", label: `−${breakdown.removed}` });
  if (breakdown.modified) badges.push({ kind: "modified", label: `~${breakdown.modified}` });
  if (breakdown.esc) badges.push({ kind: "esc", label: `↑${breakdown.esc} esc` });
  if (badges.length === 0) badges.push({ kind: "none", label: "no change" });
  return badges;
}

export function repoBreakdown(repo: RepoNode): Breakdown {
  return breakdownOf(repo.principals);
}

export function projectBreakdown(project: ProjectNode): Breakdown {
  return project.repos.reduce((acc, repo) => addBreakdown(acc, repoBreakdown(repo)), emptyBreakdown());
}

/** ADR-0009 / PD-16: repos with at least one change open automatically; if the whole Run
 * pair has zero changes anywhere, every card opens instead of everything collapsing. */
export function treeHasAnyChanges(tree: RosterTree): boolean {
  return tree.some((project) => project.repos.some((repo) => isHot(repoBreakdown(repo))));
}

export function defaultOpen(repo: RepoNode, treeHasChanges: boolean): boolean {
  return !treeHasChanges || isHot(repoBreakdown(repo));
}

/** Card header text reads in grants, never headcount — "7 grants · 2 admin" (ADR-0008).
 * Counts currently-held access, matching RepoNode's own read/write/admin count semantics
 * (a Revoke does not count — that principal no longer holds the permission as of run B). */
export function grantsText(repo: RepoNode): string {
  const total = repo.readCount + repo.writeCount + repo.adminCount;
  return `${total} grant${total === 1 ? "" : "s"} · ${repo.adminCount} admin`;
}

/** 58×6px three-stop grey gradient showing the admin/write/read split of grants (ADR-0008).
 * A repo with zero grants renders a flat --rule-row bar, not an empty element. */
export function distributionBarBackground(repo: RepoNode): string {
  const total = repo.readCount + repo.writeCount + repo.adminCount;
  if (!total) return "var(--rule-row)";
  const adminPct = (repo.adminCount / total) * 100;
  const writePct = (repo.writeCount / total) * 100;
  return (
    `linear-gradient(90deg, var(--ink) 0 ${adminPct}%, ` +
    `var(--ink-3) ${adminPct}% ${adminPct + writePct}%, ` +
    `var(--ink-faint) ${adminPct + writePct}% 100%)`
  );
}

export type AbsentSide = "A" | "B" | null;

// A repo present in one compared Snapshot's discovery but absent from the other's — distinct
// from a repo genuinely fetched on both sides with zero grants, which must not strike through.
export function absentSide(repo: RepoNode): AbsentSide {
  if (repo.statusA === null && repo.statusB !== null) return "A";
  if (repo.statusB === null && repo.statusA !== null) return "B";
  return null;
}

export interface RepoTag {
  kind: "gone" | "new" | "failed";
  label: string;
}

/** A repo absent from the Comparison side carries `absent from run N`; a repo absent only from
 * the Baseline — present for the first time as of the Comparison — carries `new in run N`. Both
 * name the Comparison run, matching the reference design. Repo arrival proper is PD-17; until
 * then this is driven entirely off repo-absence data, per PD-16's stated scope.
 *
 * Discovery (absent/new) takes priority over `fetchFailed` (PD-19) — a repo genuinely missing
 * from one side's discovery is the more significant structural fact than a fetch error on the
 * side where it *was* discovered. `fetch failed` reuses the neutral tag palette (ADR-0005): it
 * is not a diff outcome, so it may not claim the red/green hues those are reserved for. */
export function repoTag(repo: RepoNode, comparisonId: number): RepoTag | null {
  const side = absentSide(repo);
  if (side === "B") return { kind: "gone", label: `absent from run ${comparisonId}` };
  if (side === "A") return { kind: "new", label: `new in run ${comparisonId}` };
  if (repo.fetchFailed) return { kind: "failed", label: "fetch failed" };
  return null;
}

/** Project meta ("3 repositories · 14 grants") — grants, not headcount, per ADR-0008: that ADR
 * names per-project rollups explicitly and rejects counting distinct principals as an informal
 * second permission model, so this deviates from PD-16's literal example text ("12 people"). */
export function projectMeta(project: ProjectNode): string {
  const repoCount = project.repos.length;
  const grantsTotal = project.repos.reduce(
    (sum, repo) => sum + repo.readCount + repo.writeCount + repo.adminCount,
    0,
  );
  return (
    `${repoCount} ${repoCount === 1 ? "repository" : "repositories"} · ` +
    `${grantsTotal} ${grantsTotal === 1 ? "grant" : "grants"}`
  );
}
