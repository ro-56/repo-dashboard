import { describe, expect, it } from "vitest";
import { emptyStateFor } from "./emptyState";
import type { ComparisonSummary, PairStats } from "./roster";
import { t } from "./i18n/testHelpers";

const zeroPair: PairStats = {
  added: 0,
  revoked: 0,
  changed: 0,
  escalations: 0,
  reposHit: 0,
  net: 0,
};

const nonZeroPair: PairStats = {
  added: 1,
  revoked: 0,
  changed: 0,
  escalations: 0,
  reposHit: 1,
  net: 1,
};

const comparison: ComparisonSummary = {
  totalGrants: 12,
  distinctRepos: 3,
  distinctUsers: 5,
  levels: { admin: 1, write: 4, read: 7 },
};

describe("emptyStateFor", () => {
  it("returns the workspace-wide no-changes state (1h) for Changes only with zero drift", () => {
    const view = emptyStateFor(t, "changes", zeroPair, comparison, 4, 5);

    expect(view.action).toBe("show-all");
    expect(view.title).toBe("No permission changes between run 4 and run 5");
    expect(view.body).toContain("12 grants across 3 repositories");
  });

  it("singularizes grant/repository counts of exactly one", () => {
    const view = emptyStateFor(
      t,
      "changes",
      zeroPair,
      { ...comparison, totalGrants: 1, distinctRepos: 1 },
      1,
      2,
    );

    expect(view.body).toContain("1 grant across 1 repository");
  });

  it("falls back to the generic no-matches state (1i) outside Changes-only-zero-drift", () => {
    expect(emptyStateFor(t, "all", zeroPair, comparison, 1, 2).action).toBe("reset");
    expect(emptyStateFor(t, "changes", nonZeroPair, comparison, 1, 2).action).toBe("reset");
  });
});
