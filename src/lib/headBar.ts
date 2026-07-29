// Presentation logic for the 40px head bar's Run pair selectors (PD-18). Pure functions only —
// markup lives in HeadBar.svelte.

import type { ComparisonSummary, PairStats, SnapshotSummary } from "./roster";

export interface SnapshotOption {
  id: number;
  label: string;
}

function formatDate(runAt: string): string {
  return runAt.slice(0, 10);
}

/** `list_snapshots` returns newest-first; the selector labels count up from the oldest Run
 * instead, so "run 1" always names the first Run ever taken, regardless of how many have
 * been recorded since. */
export function snapshotSeqs(snapshots: SnapshotSummary[]): Map<number, number> {
  const total = snapshots.length;
  const seqs = new Map<number, number>();
  snapshots.forEach((snapshot, index) => seqs.set(snapshot.id, total - index));
  return seqs;
}

export function selectorOptions(snapshots: SnapshotSummary[], seqs: Map<number, number>): SnapshotOption[] {
  return snapshots.map((snapshot) => ({
    id: snapshot.id,
    label: `run ${seqs.get(snapshot.id)} · ${formatDate(snapshot.runAt)}`,
  }));
}

/** "N days apart · X → Y grants", or "single run · X → X grants" when both sides of the Run
 * pair are the same Snapshot. Baseline grant count is derived from the Comparison's total minus
 * the pair's signed net, rather than carried separately, since PD-17 only returns totals for the
 * Comparison side. `same` is passed in rather than re-derived from the two ids, so the page has
 * one place that decides the Run pair is a same-Snapshot roster view, shared with SummaryBar. */
export function spanNote(
  baseline: SnapshotSummary,
  comparison: SnapshotSummary,
  comparisonSummary: ComparisonSummary,
  pair: PairStats,
  same: boolean,
): string {
  const baselineGrants = comparisonSummary.totalGrants - pair.net;
  const grants = `${baselineGrants} → ${comparisonSummary.totalGrants} grants`;

  if (same) return `single run · ${grants}`;

  const days = Math.round(
    Math.abs(new Date(comparison.runAt).getTime() - new Date(baseline.runAt).getTime()) / 86_400_000,
  );
  return `${days} days apart · ${grants}`;
}
