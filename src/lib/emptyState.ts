// Presentation logic for the two "nothing to draw" states (PD-20). Pure functions only —
// markup lives in EmptyState.svelte.

import type { ComparisonSummary, PairStats } from "./roster";
import type { ViewMode } from "./filterBar";
import type { Translate } from "./i18n/translate";

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
  t: Translate,
  mode: ViewMode,
  pair: PairStats,
  comparison: ComparisonSummary,
  baselineSeq: number,
  comparisonSeq: number,
): EmptyStateView {
  const zeroChanges = pair.added === 0 && pair.revoked === 0 && pair.changed === 0;

  if (mode === "changes" && zeroChanges) {
    return {
      title: t("emptyState.noChangesTitle", { values: { baseline: baselineSeq, comparison: comparisonSeq } }),
      body: t("emptyState.noChangesBody", {
        values: { grants: comparison.totalGrants, repos: comparison.distinctRepos, comparison: comparisonSeq },
      }),
      actionLabel: t("emptyState.showAllAccess"),
      action: "show-all",
    };
  }

  return {
    title: t("emptyState.noMatchesTitle"),
    body: t("emptyState.noMatchesBody"),
    actionLabel: t("emptyState.resetFilters"),
    action: "reset",
  };
}
