import { describe, expect, it } from "vitest";
import { canApply, deltaChips, selectorOptions, snapshotSeqs, spanNote } from "./headBar";
import type { ComparisonSummary, PairStats, SnapshotSummary } from "./roster";
import { t } from "./i18n/testHelpers";

describe("canApply", () => {
  it("is true when there's staged work and editing is enabled", () => {
    expect(canApply(1, true)).toBe(true);
  });

  it("is false when there's nothing staged, even if editing is enabled", () => {
    expect(canApply(0, true)).toBe(false);
  });

  it("is false when editing is disabled, even with staged work", () => {
    expect(canApply(1, false)).toBe(false);
  });

  it("is false when there's nothing staged and editing is disabled", () => {
    expect(canApply(0, false)).toBe(false);
  });
});

const zeroPair: PairStats = { added: 0, revoked: 0, changed: 0, escalations: 0, reposHit: 0, net: 0 };

describe("deltaChips", () => {
  it("emits a single no-change chip when the pair has no diff at all", () => {
    expect(deltaChips(t, zeroPair)).toEqual([{ key: "none", value: "", label: "no change", tone: "none" }]);
  });

  it("emits one chip per non-zero count, translated", () => {
    const chips = deltaChips(t, { added: 2, revoked: 1, changed: 3, escalations: 1, reposHit: 1, net: 1 });
    expect(chips).toEqual([
      { key: "added", value: "+2", label: "added", tone: "added" },
      { key: "revoked", value: "−1", label: "revoked", tone: "revoked" },
      { key: "changed", value: "~3", label: "changed", tone: "changed" },
      { key: "escalations", value: "↑1", label: "escalations", tone: "escalation" },
    ]);
  });
});

describe("selectorOptions", () => {
  it("labels each snapshot with its 1-indexed sequence number and date", () => {
    const snapshots: SnapshotSummary[] = [
      { id: 2, runAt: "2026-08-02T10:00:00Z" },
      { id: 1, runAt: "2026-08-01T10:00:00Z" },
    ];
    const seqs = snapshotSeqs(snapshots);
    expect(selectorOptions(t, snapshots, seqs)).toEqual([
      { id: 2, label: "run 2 · 2026-08-02" },
      { id: 1, label: "run 1 · 2026-08-01" },
    ]);
  });
});

describe("spanNote", () => {
  const comparisonSummary: ComparisonSummary = {
    totalGrants: 12,
    distinctRepos: 3,
    distinctUsers: 5,
    levels: { admin: 1, write: 4, read: 7 },
  };
  const pair: PairStats = { added: 2, revoked: 0, changed: 0, escalations: 0, reposHit: 1, net: 2 };
  const baseline: SnapshotSummary = { id: 1, runAt: "2026-08-01T00:00:00Z" };
  const comparison: SnapshotSummary = { id: 2, runAt: "2026-08-03T00:00:00Z" };

  it("reads 'single run' when both sides of the pair are the same snapshot", () => {
    expect(spanNote(t, baseline, comparison, comparisonSummary, pair, true)).toBe("single run · 10 → 12 grants");
  });

  it("reads 'N days apart' when the pair spans two different snapshots", () => {
    expect(spanNote(t, baseline, comparison, comparisonSummary, pair, false)).toBe("2 days apart · 10 → 12 grants");
  });
});
