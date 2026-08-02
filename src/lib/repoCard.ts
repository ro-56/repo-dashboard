// Presentation logic for repo cards and project sections (ADR-0008, ADR-0009). Pure functions
// only — markup and colour live in RepoCard.svelte / ProjectSection.svelte's scoped styles.

import type { PrincipalEntry, ProjectNode, RepoNode, RosterTree } from "./roster";

export interface CountBadge {
  kind: "added" | "removed" | "modified" | "esc" | "none";
  label: string;
  title?: string;
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
  if (breakdown.added)
    badges.push({ kind: "added", label: `+${breakdown.added}`, title: `${breakdown.added} grants added since baseline` });
  if (breakdown.removed)
    badges.push({ kind: "removed", label: `−${breakdown.removed}`, title: `${breakdown.removed} grants revoked since baseline` });
  if (breakdown.modified)
    badges.push({
      kind: "modified",
      label: `~${breakdown.modified}`,
      title: `${breakdown.modified} level changes since baseline`,
    });
  if (breakdown.esc)
    badges.push({
      kind: "esc",
      label: `↑${breakdown.esc} esc`,
      title: `${breakdown.esc} of those changes are escalations (permission increased)`,
    });
  if (badges.length === 0) badges.push({ kind: "none", label: "no change" });
  return badges;
}

export function repoBreakdown(repo: RepoNode): Breakdown {
  return breakdownOf(repo.principals);
}

export function projectBreakdown(project: ProjectNode): Breakdown {
  return project.repos.reduce((acc, repo) => addBreakdown(acc, repoBreakdown(repo)), emptyBreakdown());
}

/** Whether a repo carries at least one Grant/Revoke/LevelChange — shared by the default-expand
 * rule (below) and the Changes only tab's card-level filter (filterBar.ts), so the two can't
 * drift apart on what counts as "changed" (PD-20). */
export function repoHasChanges(repo: RepoNode): boolean {
  return isHot(repoBreakdown(repo));
}

/** ADR-0009 / PD-16: repos with at least one change open automatically; if the whole Run
 * pair has zero changes anywhere, every card opens instead of everything collapsing. */
export function treeHasAnyChanges(tree: RosterTree): boolean {
  return tree.some((project) => project.repos.some((repo) => isHot(repoBreakdown(repo))));
}

export function defaultOpen(repo: RepoNode, treeHasChanges: boolean): boolean {
  return !treeHasChanges || repoHasChanges(repo);
}

/** Whether a card renders open right now: the default-expand rule XOR'd against the user's own
 * toggle (PD-16). Shared by RepoCard.svelte, the bulk expand/collapse scope check, and the
 * label decision (filterBar.ts) — same formula, one place. */
export function isRepoOpen(repo: RepoNode, treeHasChanges: boolean, toggled: boolean): boolean {
  return defaultOpen(repo, treeHasChanges) !== toggled;
}

/** Card header text reads in grants, never headcount — "7 grants · 2 admin" (ADR-0008).
 * Counts currently-held access, matching RepoNode's own read/write/admin count semantics
 * (a Revoke does not count — that principal no longer holds the permission as of run B). */
export function grantsText(repo: RepoNode): string {
  const total = repo.readCount + repo.writeCount + repo.adminCount;
  return `${total} grant${total === 1 ? "" : "s"} · ${repo.adminCount} admin`;
}

export interface DistributionSplit {
  admin: number;
  write: number;
  read: number;
}

/** Per-segment admin/write/read percentages (summing to 100) for the repo card's distribution
 * bar (ADR-0008), rendered as 3 literal segments in RepoCard.svelte. `null` signals a repo with
 * zero grants, which the component renders as a single flat neutral bar, not empty segments. */
export function distributionSplit(repo: RepoNode): DistributionSplit | null {
  const total = repo.readCount + repo.writeCount + repo.adminCount;
  if (!total) return null;
  return {
    admin: (repo.adminCount / total) * 100,
    write: (repo.writeCount / total) * 100,
    read: (repo.readCount / total) * 100,
  };
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
  kind: "gone" | "new" | "failed" | "project-failed";
  label: string;
  title: string;
}

const REPO_TAG_TITLE: Record<RepoTag["kind"], string> = {
  gone: "Repo not found",
  new: "Repo not present in the baseline",
  failed: "Repo's permissions couldn't be fetched",
  "project-failed": "Project's permissions couldn't be fetched",
};

/** The `.bar-cell` distribution bar's `title` tooltip: the actual admin/write/read counts behind
 * the three proportional segments — most useful below the 1300px reflow breakpoint (ADR-0009),
 * where the adjacent `.grants` text cell that also carries these numbers is hidden. */
export function barCellTooltip(repo: RepoNode): string {
  return `${repo.adminCount} admin · ${repo.writeCount} write · ${repo.readCount} read`;
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
  if (side === "B") return { kind: "gone", label: `absent from run ${comparisonId}`, title: REPO_TAG_TITLE.gone };
  if (side === "A") return { kind: "new", label: `new in run ${comparisonId}`, title: REPO_TAG_TITLE.new };
  if (repo.fetchFailed) return { kind: "failed", label: "fetch failed", title: REPO_TAG_TITLE.failed };
  return null;
}

/** All header tags for a repo card: the priority-ordered gone/new/failed tag (`repoTag`) plus an
 * independent `project fetch failed` tag when the owning Project's own fetch failed (ADR-0012).
 * The two failures are orthogonal (a repo's own fetch and its Project's fetch fail or succeed
 * independently), so `project fetch failed` is additive rather than subject to `repoTag`'s
 * priority order — both tags can render together (PD-40). */
export function repoTags(repo: RepoNode, comparisonId: number): RepoTag[] {
  const tags: RepoTag[] = [];
  const primary = repoTag(repo, comparisonId);
  if (primary) tags.push(primary);
  if (repo.projectFetchFailed)
    tags.push({ kind: "project-failed", label: "project fetch failed", title: REPO_TAG_TITLE["project-failed"] });
  return tags;
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
