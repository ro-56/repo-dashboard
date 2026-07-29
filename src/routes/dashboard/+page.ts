import { invoke } from "@tauri-apps/api/core";
import type { ComparisonSummary, PairStats, RosterTree, RosterTreeResult, SnapshotSummary } from "$lib/roster";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ url }) => {
  const snapshots = await invoke<SnapshotSummary[]>("list_snapshots");

  const latestId = snapshots[0]?.id ?? null;
  const previousId = snapshots[1]?.id ?? latestId;

  const paramBaseline = url.searchParams.get("baseline");
  const paramComparison = url.searchParams.get("comparison");

  const baselineId = paramBaseline !== null ? Number(paramBaseline) : previousId;
  const comparisonId = paramComparison !== null ? Number(paramComparison) : latestId;

  let tree: RosterTree = [];
  let comparison: ComparisonSummary | null = null;
  let pair: PairStats | null = null;

  if (baselineId !== null && comparisonId !== null) {
    const result = await invoke<RosterTreeResult>("get_roster_tree", {
      snapshotAId: baselineId,
      snapshotBId: comparisonId,
    });
    tree = result.tree;
    comparison = result.comparison;
    pair = result.pair;
  }

  return { snapshots, baselineId, comparisonId, tree, comparison, pair };
};
