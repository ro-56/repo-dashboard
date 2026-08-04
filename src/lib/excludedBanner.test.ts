import { describe, expect, it } from "vitest";
import { excludedBannerCount, excludedPrincipals } from "./excludedBanner";
import type { PrincipalEntry, ProjectNode, RepoNode, RosterTree } from "./roster";
import { t } from "./i18n/testHelpers";

function principal(overrides: Partial<PrincipalEntry> = {}): PrincipalEntry {
  return {
    principal: { id: "u1", label: "Alice" },
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

function tree(repos: RepoNode[]): RosterTree {
  const project: ProjectNode = { repoProject: "TEAM", repos };
  return [project];
}

describe("excludedPrincipals", () => {
  it("is empty when the exclusion set is empty", () => {
    const t1 = tree([repo({ principals: [principal()] })]);
    expect(excludedPrincipals(t1, new Set())).toEqual([]);
  });

  it("labels each excluded id from a matching row in the tree", () => {
    const t1 = tree([
      repo({
        principals: [
          principal({ principal: { id: "u1", label: "Alice" } }),
          principal({ principal: { id: "g1", label: "Auditors" }, accessType: { type: "Group" } }),
        ],
      }),
    ]);
    expect(excludedPrincipals(t1, new Set(["u1", "g1"]))).toEqual([
      { id: "u1", label: "Alice" },
      { id: "g1", label: "Auditors" },
    ]);
  });

  it("labels a hidden Group from its own Group row, not a Member row it cascades to", () => {
    const t1 = tree([
      repo({
        principals: [
          principal({ principal: { id: "g1", label: "Auditors" }, accessType: { type: "Group" } }),
          principal({ principal: { id: "u2", label: "Bob" }, accessType: { type: "Member", group_id: "g1" } }),
        ],
      }),
    ]);
    expect(excludedPrincipals(t1, new Set(["g1"]))).toEqual([{ id: "g1", label: "Auditors" }]);
  });

  it("falls back to the id itself when no row in the tree matches it", () => {
    const t1 = tree([repo({ principals: [principal({ principal: { id: "u1", label: "Alice" } })] })]);
    expect(excludedPrincipals(t1, new Set(["ghost"]))).toEqual([{ id: "ghost", label: "ghost" }]);
  });

  it("returns exactly one entry per excluded id even if it appears on many rows", () => {
    const t1 = tree([
      repo({
        repo: "widget-a",
        principals: [principal({ principal: { id: "u1", label: "Alice" } })],
      }),
      repo({
        repo: "widget-b",
        principals: [
          principal({ principal: { id: "u1", label: "Alice" }, scope: "Project" }),
          principal({ principal: { id: "u1", label: "Alice" }, accessType: { type: "Member", group_id: "g1" } }),
        ],
      }),
    ]);
    expect(excludedPrincipals(t1, new Set(["u1"]))).toEqual([{ id: "u1", label: "Alice" }]);
  });
});

describe("excludedBannerCount", () => {
  it("pluralizes for exactly one excluded Principal", () => {
    expect(excludedBannerCount(t, new Set(["u1"]))).toBe("1 principal hidden this session");
  });

  it("pluralizes for more than one excluded Principal", () => {
    expect(excludedBannerCount(t, new Set(["u1", "g1"]))).toBe("2 principals hidden this session");
  });
});
