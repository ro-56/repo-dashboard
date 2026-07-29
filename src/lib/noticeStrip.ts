// Presentation logic for the 25px structural notice strip (PD-19, ADR-0005). Pure functions
// only — markup and colour live in NoticeStrip.svelte's scoped styles. Neither Fetch failure
// nor Unresolvable membership may claim a hue of its own (ADR-0005), so both surface here as
// prose alongside the tags already added in repoCard.ts/rosterRow.ts.

import type { RepoNode, RosterTree } from "./roster";
import { isUnresolvedGroup } from "./rosterRow";

interface Noun {
  singular: string;
  plural: string;
}

/** Names up to three items then counts the rest — `core-api, auth-service, event-bus and 4
 * more repositories` — so the strip stays one line at any workspace size. */
function joinNamed(items: string[], noun: Noun): string {
  const shown = items.slice(0, 3);
  const restCount = items.length - shown.length;
  if (restCount > 0) return `${shown.join(", ")} and ${restCount} more ${noun.plural}`;
  if (shown.length === 1) return shown[0];
  return `${shown.slice(0, -1).join(", ")} and ${shown[shown.length - 1]}`;
}

function allRepos(tree: RosterTree): RepoNode[] {
  return tree.flatMap((project) => project.repos);
}

function reposByDiscovery(tree: RosterTree, state: "absent" | "arrived"): string[] {
  return allRepos(tree)
    .filter((r) => r.discoveryState === state)
    .map((r) => r.repo)
    .sort((a, b) => a.localeCompare(b));
}

function fetchFailedRepos(tree: RosterTree): string[] {
  return allRepos(tree)
    .filter((r) => r.fetchFailed)
    .map((r) => r.repo)
    .sort((a, b) => a.localeCompare(b));
}

/** Distinct labels of user Principals (never a Group's own row) whose `workspaceState` matches
 * `state`, deduped by principal id since the same person can appear on several repo rows. */
function workspaceUsers(tree: RosterTree, state: "departed" | "arrived"): string[] {
  const seen = new Map<string, string>();
  for (const project of tree) {
    for (const repo of project.repos) {
      for (const entry of repo.principals) {
        if (entry.workspaceState === state) {
          seen.set(entry.principal.id, entry.principal.label);
        }
      }
    }
  }
  return [...seen.values()].sort((a, b) => a.localeCompare(b));
}

/** Distinct labels of Group Principals whose membership could not be resolved, deduped by
 * group id since the same group can grant access to several repos. */
function unresolvedGroups(tree: RosterTree): string[] {
  const seen = new Map<string, string>();
  for (const project of tree) {
    for (const repo of project.repos) {
      for (const entry of repo.principals) {
        if (isUnresolvedGroup(entry)) {
          seen.set(entry.principal.id, entry.principal.label);
        }
      }
    }
  }
  return [...seen.values()].sort((a, b) => a.localeCompare(b));
}

const REPO_NOUN: Noun = { singular: "repository", plural: "repositories" };
const USER_NOUN: Noun = { singular: "user", plural: "users" };
const GROUP_NOUN: Noun = { singular: "group", plural: "groups" };

function verb(count: number, singular: string, plural: string): string {
  return count > 1 ? plural : singular;
}

/** Narrates every structural fact about the current Run pair — repo absence, repo arrival,
 * workspace departure, workspace arrival, fetch failures, unresolvable memberships — in that
 * order, one sentence per category. Returns `null` when there is nothing to report, so the
 * strip renders nothing at all (per its acceptance criteria) rather than an empty bar.
 *
 * Fetch failure and Unresolvable membership (CONTEXT.md) are per-Snapshot data-quality facts,
 * not diff facts — unlike the other four categories, they are meaningful even when `same` is
 * true, so they are never skipped by the same-Snapshot branch below. The other four *are*
 * diff-shaped and, by construction, always compute empty in the same-Snapshot case (there is
 * no baseline to differ against), so no explicit guard is needed for them either. */
export function structuralNotice(tree: RosterTree, same: boolean, comparisonSeq: number): string | null {
  const sentences: string[] = [];

  if (same) {
    sentences.push(
      `Both sides point at run ${comparisonSeq}. This is that run's access list on its own — ` +
        `no diff is layered on top of it.`,
    );
  }

  const absentRepos = reposByDiscovery(tree, "absent");
  if (absentRepos.length) {
    sentences.push(
      `${joinNamed(absentRepos, REPO_NOUN)} ${verb(absentRepos.length, "is", "are")} absent from the comparison run.`,
    );
  }

  const arrivedRepos = reposByDiscovery(tree, "arrived");
  if (arrivedRepos.length) {
    const appear = verb(arrivedRepos.length, "appears", "appear");
    sentences.push(`${joinNamed(arrivedRepos, REPO_NOUN)} ${appear} for the first time in the comparison run.`);
  }

  const departed = workspaceUsers(tree, "departed");
  if (departed.length) {
    const hold = verb(departed.length, "holds", "hold");
    sentences.push(`${joinNamed(departed, USER_NOUN)} no longer ${hold} access to any repository.`);
  }

  const arrived = workspaceUsers(tree, "arrived");
  if (arrived.length) {
    sentences.push(
      `${joinNamed(arrived, USER_NOUN)} ${verb(arrived.length, "is", "are")} new to the audit.`,
    );
  }

  const fetchFailed = fetchFailedRepos(tree);
  if (fetchFailed.length) {
    sentences.push(`${joinNamed(fetchFailed, REPO_NOUN)} could not be fetched this run.`);
  }

  const unresolved = unresolvedGroups(tree);
  if (unresolved.length) {
    sentences.push(`${joinNamed(unresolved, GROUP_NOUN)} membership could not be resolved.`);
  }

  return sentences.length ? sentences.join("  ") : null;
}
