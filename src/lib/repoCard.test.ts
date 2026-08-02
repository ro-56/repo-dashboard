import { describe, expect, it } from "vitest";
import {
  absentSide,
  barCellTooltip,
  countBadges,
  defaultOpen,
  distributionSplit,
  grantsText,
  isRepoOpen,
  projectMeta,
  repoBreakdown,
  repoHasChanges,
  repoTags,
  treeHasAnyChanges,
} from "./repoCard";
import type { PrincipalEntry, ProjectNode, RepoNode, RosterTree } from "./roster";

function principal(overrides: Partial<PrincipalEntry> = {}): PrincipalEntry {
  return {
    principal: { id: "u1", label: "alice" },
    accessType: { type: "Direct" },
    scope: "Repo",
    permission: "Read",
    diffStatus: { status: "None" },
    membersResolved: null,
    ...overrides,
  };
}

function repo(overrides: Partial<RepoNode> = {}): RepoNode {
  return {
    repoProject: "TEAM",
    repo: "widget",
    statusA: "Ok",
    statusB: "Ok",
    discoveryState: "present",
    fetchFailed: false,
    projectFetchFailed: false,
    principals: [],
    readCount: 0,
    writeCount: 0,
    adminCount: 0,
    changeCount: 0,
    ...overrides,
  };
}

describe("repoBreakdown / countBadges", () => {
  it("counts Grant/Revoke/LevelChange and flags escalations", () => {
    const r = repo({
      principals: [
        principal({ diffStatus: { status: "Grant" } }),
        principal({ diffStatus: { status: "Revoke" } }),
        principal({
          diffStatus: { status: "LevelChange", from: "Read", to: "Admin", kind: "Escalation" },
        }),
      ],
    });
    const breakdown = repoBreakdown(r);
    expect(breakdown).toEqual({ added: 1, removed: 1, modified: 1, esc: 1 });
    expect(countBadges(breakdown)).toEqual([
      { kind: "added", label: "+1", title: "1 grants added since baseline" },
      { kind: "removed", label: "−1", title: "1 grants revoked since baseline" },
      { kind: "modified", label: "~1", title: "1 level changes since baseline" },
      { kind: "esc", label: "↑1 esc", title: "1 of those changes are escalations (permission increased)" },
    ]);
  });

  it("renders 'no change' when nothing in the repo differs", () => {
    expect(countBadges(repoBreakdown(repo()))).toEqual([{ kind: "none", label: "no change" }]);
  });
});

describe("repoHasChanges / treeHasAnyChanges / defaultOpen / isRepoOpen", () => {
  it("a repo with any diff is 'hot'; one with none is not", () => {
    expect(repoHasChanges(repo({ principals: [principal({ diffStatus: { status: "Grant" } })] }))).toBe(
      true,
    );
    expect(repoHasChanges(repo())).toBe(false);
  });

  it("tree has changes if any repo anywhere does", () => {
    const tree: RosterTree = [
      { repoProject: "TEAM", repos: [repo(), repo({ principals: [principal({ diffStatus: { status: "Grant" } })] })] },
    ];
    expect(treeHasAnyChanges(tree)).toBe(true);
    expect(treeHasAnyChanges([{ repoProject: "TEAM", repos: [repo()] }])).toBe(false);
  });

  it("defaults every card open when the whole pair has zero changes, else only hot ones", () => {
    const hot = repo({ principals: [principal({ diffStatus: { status: "Grant" } })] });
    const cold = repo();
    expect(defaultOpen(cold, false)).toBe(true);
    expect(defaultOpen(cold, true)).toBe(false);
    expect(defaultOpen(hot, true)).toBe(true);
  });

  it("isRepoOpen XORs the default against the user's own toggle", () => {
    const cold = repo();
    expect(isRepoOpen(cold, true, false)).toBe(false);
    expect(isRepoOpen(cold, true, true)).toBe(true);
  });
});

describe("grantsText / projectMeta", () => {
  it("sums read/write/admin counts and singularizes at one", () => {
    expect(grantsText(repo({ readCount: 3, writeCount: 2, adminCount: 1 }))).toBe("6 grants · 1 admin");
    expect(grantsText(repo({ readCount: 1 }))).toBe("1 grant · 0 admin");
  });

  it("summarizes a project's repo count and total grants", () => {
    const project: ProjectNode = {
      repoProject: "TEAM",
      repos: [repo({ readCount: 1 }), repo({ writeCount: 1, adminCount: 1 })],
    };
    expect(projectMeta(project)).toBe("2 repositories · 3 grants");
  });
});

describe("distributionSplit", () => {
  it("splits admin/write/read counts into percentages summing to 100", () => {
    expect(distributionSplit(repo({ adminCount: 2, writeCount: 1, readCount: 1 }))).toEqual({
      admin: 50,
      write: 25,
      read: 25,
    });
  });

  it("returns null for a repo with zero grants", () => {
    expect(distributionSplit(repo())).toBeNull();
  });
});

describe("barCellTooltip", () => {
  it("reports admin/write/read counts, not percentages", () => {
    expect(barCellTooltip(repo({ adminCount: 2, writeCount: 1, readCount: 1 }))).toBe(
      "2 admin · 1 write · 1 read",
    );
  });

  it("still reports zeros for a repo with no grants, rather than omitting the tooltip", () => {
    expect(barCellTooltip(repo())).toBe("0 admin · 0 write · 0 read");
  });
});

describe("absentSide", () => {
  it("flags a repo missing from the Comparison side as absent from B, and vice versa", () => {
    expect(absentSide(repo({ statusA: "Ok", statusB: null }))).toBe("B");
    expect(absentSide(repo({ statusA: null, statusB: "Ok" }))).toBe("A");
    expect(absentSide(repo({ statusA: "Ok", statusB: "Ok" }))).toBe(null);
  });
});

describe("repoTags", () => {
  it("is empty for an ordinary repo with no discovery/fetch issues", () => {
    expect(repoTags(repo(), 5)).toEqual([]);
  });

  it("names the Comparison run for absent/new discovery tags", () => {
    expect(repoTags(repo({ statusA: "Ok", statusB: null }), 5)).toEqual([
      { kind: "gone", label: "absent from run 5", title: "Repo not found" },
    ]);
    expect(repoTags(repo({ statusA: null, statusB: "Ok" }), 5)).toEqual([
      { kind: "new", label: "new in run 5", title: "Repo not present in the baseline" },
    ]);
  });

  it("discovery absence takes priority over the repo's own fetchFailed (PD-19)", () => {
    expect(repoTags(repo({ statusA: "Ok", statusB: null, fetchFailed: true }), 5)).toEqual([
      { kind: "gone", label: "absent from run 5", title: "Repo not found" },
    ]);
  });

  it("shows 'fetch failed' when only the repo's own fetch failed", () => {
    expect(repoTags(repo({ fetchFailed: true }), 5)).toEqual([
      { kind: "failed", label: "fetch failed", title: "Repo's permissions couldn't be fetched" },
    ]);
  });

  it("shows 'project fetch failed' when only the owning Project's fetch failed", () => {
    expect(repoTags(repo({ projectFetchFailed: true }), 5)).toEqual([
      { kind: "project-failed", label: "project fetch failed", title: "Project's permissions couldn't be fetched" },
    ]);
  });

  it("shows both fetch-failure tags together, distinctly, when both are true", () => {
    expect(repoTags(repo({ fetchFailed: true, projectFetchFailed: true }), 5)).toEqual([
      { kind: "failed", label: "fetch failed", title: "Repo's permissions couldn't be fetched" },
      { kind: "project-failed", label: "project fetch failed", title: "Project's permissions couldn't be fetched" },
    ]);
  });

  it("still surfaces 'project fetch failed' alongside a discovery tag", () => {
    expect(repoTags(repo({ statusA: null, statusB: "Ok", projectFetchFailed: true }), 5)).toEqual([
      { kind: "new", label: "new in run 5", title: "Repo not present in the baseline" },
      { kind: "project-failed", label: "project fetch failed", title: "Project's permissions couldn't be fetched" },
    ]);
  });
});
