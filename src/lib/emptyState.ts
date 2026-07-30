// Presentation logic for the two "nothing to draw" states (PD-20). Pure functions only —
// markup lives in EmptyState.svelte.

import type { ComparisonSummary, PairStats } from "./roster";
import type { ViewMode } from "./filterBar";

export interface EmptyStateView {
  title: string;
  body: string;
  actionLabel: string;
  action: "show-all" | "reset";
}

/** Which of the two states applies: the workspace-wide no-changes state (`1h`) when Changes
 * only has zero drift anywhere in the Run pair, or the generic no-matches fallback (`1i`) for
 * every other way a view can end up empty. Only the tab exists today, so `1i` is reachable
 * only from a genuinely empty roster (e.g. a workspace with no repos yet) — it's kept generic
 * so the project/repo/level/state/search filters (out of scope here) can reuse it later without
 * a relayout, per the ticket's stated scope. */
export function emptyStateFor(
  mode: ViewMode,
  pair: PairStats,
  comparison: ComparisonSummary,
  baselineSeq: number,
  comparisonSeq: number,
): EmptyStateView {
  const zeroChanges = pair.added === 0 && pair.revoked === 0 && pair.changed === 0;

  if (mode === "changes" && zeroChanges) {
    const grantsWord = comparison.totalGrants === 1 ? "grant" : "grants";
    const repoWord = comparison.distinctRepos === 1 ? "repository" : "repositories";
    return {
      title: `No permission changes between run ${baselineSeq} and run ${comparisonSeq}`,
      body:
        `Both runs grant exactly the same ${comparison.totalGrants} ${grantsWord} across ` +
        `${comparison.distinctRepos} ${repoWord}. Switch to All access to read run ${comparisonSeq}'s roster instead.`,
      actionLabel: "Show all access",
      action: "show-all",
    };
  }

  return {
    title: "Nothing matches these filters",
    body: "Widen the current view to see the rest of the roster.",
    actionLabel: "Reset filters",
    action: "reset",
  };
}
