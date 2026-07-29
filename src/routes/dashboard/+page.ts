import { invoke } from "@tauri-apps/api/core";
import type { ComparisonSummary, PairStats, RosterTree, RosterTreeResult, SnapshotSummary } from "$lib/roster";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ url }) => {
  const snapshots = await invoke<SnapshotSummary[]>("list_snapshots");

  const latestId = snapshots[0]?.id ?? null;
  const previousId = snapshots[1]?.id ?? latestId;

  const paramA = url.searchParams.get("snapshot_a");
  const paramB = url.searchParams.get("snapshot_b");

  const snapshotAId = paramA !== null ? Number(paramA) : previousId;
  const snapshotBId = paramB !== null ? Number(paramB) : latestId;

  let tree: RosterTree = [];
  let comparison: ComparisonSummary | null = null;
  let pair: PairStats | null = null;

  if (snapshotAId !== null && snapshotBId !== null) {
    const result = await invoke<RosterTreeResult>("get_roster_tree", { snapshotAId, snapshotBId });
    tree = result.tree;
    comparison = result.comparison;
    pair = result.pair;
  }

  return { snapshots, snapshotAId, snapshotBId, tree, comparison, pair };
};
