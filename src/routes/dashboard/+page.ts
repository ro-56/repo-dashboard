import { invoke } from "@tauri-apps/api/core";
import type { RosterTree, SnapshotSummary } from "$lib/roster";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ url }) => {
  const snapshots = await invoke<SnapshotSummary[]>("list_snapshots");

  const latestId = snapshots[0]?.id ?? null;
  const previousId = snapshots[1]?.id ?? latestId;

  const paramA = url.searchParams.get("snapshot_a");
  const paramB = url.searchParams.get("snapshot_b");

  const snapshotAId = paramA !== null ? Number(paramA) : previousId;
  const snapshotBId = paramB !== null ? Number(paramB) : latestId;

  const tree =
    snapshotAId !== null && snapshotBId !== null
      ? await invoke<RosterTree>("get_roster_tree", { snapshotAId, snapshotBId })
      : [];

  return { snapshots, snapshotAId, snapshotBId, tree };
};
