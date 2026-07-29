// Presentation logic for the 44px summary bar (PD-18). Pure functions only — markup and colour
// live in SummaryBar.svelte's scoped styles, keyed off the `tone` returned here.

import type { ComparisonSummary, PairStats } from "./roster";
import { METER } from "./rosterRow";

export type SummaryTone = "neutral" | "admin" | "write" | "read" | "added" | "removed" | "modified" | "esc" | "muted";

export interface SummaryStat {
  key: string;
  value: string;
  label: string;
  tone: SummaryTone;
}

export function leftClusterLabel(comparisonSeq: number): string {
  return `current state · run ${comparisonSeq}`;
}

export function rightClusterLabel(same: boolean, baselineSeq: number): string {
  return same ? "no baseline — roster only" : `delta vs run ${baselineSeq}`;
}

/** Current state cluster — always the Comparison Snapshot's own figures, per ADR-0008 (grants,
 * not headcount). Never affected by the same-Snapshot case; that only blanks the delta cluster. */
export function currentStateStats(comparison: ComparisonSummary): SummaryStat[] {
  return [
    { key: "grants", value: String(comparison.totalGrants), label: "grants", tone: "neutral" },
    { key: "repos", value: String(comparison.distinctRepos), label: "repos", tone: "neutral" },
    { key: "users", value: String(comparison.distinctUsers), label: "users", tone: "neutral" },
    { key: "admin", value: String(comparison.levels.admin), label: `${METER.Admin} admin`, tone: "admin" },
    { key: "write", value: String(comparison.levels.write), label: `${METER.Write} write`, tone: "write" },
    { key: "read", value: String(comparison.levels.read), label: `${METER.Read} read`, tone: "read" },
  ];
}

function signed(n: number): string {
  if (n > 0) return `+${n}`;
  if (n < 0) return String(n);
  return "+0";
}

/** Delta cluster — em-dashes in `muted` tone when the Run pair is a single Snapshot compared to
 * itself, per PD-18's acceptance criteria, rather than showing zeroes. */
export function deltaStats(pair: PairStats, same: boolean): SummaryStat[] {
  if (same) {
    return ["added", "revoked", "changed", "escalations", "repos hit", "net"].map((label) => ({
      key: label,
      value: "—",
      label,
      tone: "muted",
    }));
  }

  return [
    { key: "added", value: `+${pair.added}`, label: "added", tone: "added" },
    { key: "revoked", value: `−${pair.revoked}`, label: "revoked", tone: "removed" },
    { key: "changed", value: `~${pair.changed}`, label: "changed", tone: "modified" },
    { key: "escalations", value: `↑${pair.escalations}`, label: "escalations", tone: "esc" },
    { key: "reposHit", value: String(pair.reposHit), label: "repos hit", tone: "neutral" },
    { key: "net", value: signed(pair.net), label: "net", tone: "neutral" },
  ];
}
