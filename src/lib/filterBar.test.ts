import { describe, expect, it } from "vitest";
import {
  countNote,
  isEntryVisible,
  isSearchMatch,
  matchesSearch,
  viewCounts,
  visibleRepos,
  type ViewCounts,
} from "./filterBar";
import type { PrincipalEntry, ProjectNode, RepoNode } from "./roster";
import { t } from "./i18n/testHelpers";

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

function project(repos: RepoNode[]): ProjectNode {
  return { repoProject: "TEAM", repos };
}

describe("countNote", () => {
  it("renders the live/changed/rows counts", () => {
    const counts: ViewCounts = { live: 4, changed: 2, rows: 6 };
    expect(countNote(t, counts)).toBe("4 live · 2 changed · 6 rows");
  });
});

describe("matchesSearch", () => {
  it("matches any repo when the query is blank or whitespace", () => {
    const r = repo({ principals: [principal({ principal: { id: "u1", label: "Alice Smith" } })] });
    expect(matchesSearch(r, "")).toBe(true);
    expect(matchesSearch(r, "   ")).toBe(true);
  });

  it("matches case-insensitively as a substring against a Direct principal's label", () => {
    const r = repo({ principals: [principal({ principal: { id: "u1", label: "Bob Smith" } })] });
    expect(matchesSearch(r, "bob")).toBe(true);
    expect(matchesSearch(r, "BOB")).toBe(true);
    expect(matchesSearch(r, "smith")).toBe(true);
  });

  it("matches against a Group principal's label", () => {
    const r = repo({
      principals: [
        principal({ principal: { id: "g1", label: "Auditors" }, accessType: { type: "Group" } }),
      ],
    });
    expect(matchesSearch(r, "audit")).toBe(true);
  });

  it("matches against a Member principal's label", () => {
    const r = repo({
      principals: [
        principal({
          principal: { id: "u2", label: "Carol Jones" },
          accessType: { type: "Member", group_id: "g1" },
        }),
      ],
    });
    expect(matchesSearch(r, "carol")).toBe(true);
  });

  it("does not match when the query matches no principal in the repo", () => {
    const r = repo({ principals: [principal({ principal: { id: "u1", label: "Alice Smith" } })] });
    expect(matchesSearch(r, "zzz")).toBe(false);
  });
});

describe("isSearchMatch", () => {
  it("is false when search is inactive, unlike matchesSearch's vacuous true", () => {
    const r = repo({ principals: [principal({ principal: { id: "u1", label: "Alice" } })] });
    expect(matchesSearch(r, "")).toBe(true);
    expect(isSearchMatch(r, "")).toBe(false);
    expect(isSearchMatch(r, "   ")).toBe(false);
  });

  it("is true only when an active query actually matches the repo", () => {
    const r = repo({ principals: [principal({ principal: { id: "u1", label: "Alice" } })] });
    expect(isSearchMatch(r, "alice")).toBe(true);
    expect(isSearchMatch(r, "zzz")).toBe(false);
  });
});

describe("isEntryVisible", () => {
  it("all mode: a row is visible only if it matches an active search", () => {
    const alice = principal({ principal: { id: "u1", label: "Alice" } });
    const zed = principal({ principal: { id: "u2", label: "Zed" } });
    expect(isEntryVisible(alice, "all", "alice")).toBe(true);
    expect(isEntryVisible(zed, "all", "alice")).toBe(false);
    expect(isEntryVisible(zed, "all", "")).toBe(true);
  });

  it("changes mode: a row needs both a diff and a search match", () => {
    const matchedNoDiff = principal({ principal: { id: "u1", label: "Alice" } });
    const matchedWithDiff = principal({ principal: { id: "u1", label: "Alice" }, diffStatus: { status: "Grant" } });
    const diffNoMatch = principal({ principal: { id: "u2", label: "Bob" }, diffStatus: { status: "Grant" } });
    expect(isEntryVisible(matchedNoDiff, "changes", "alice")).toBe(false);
    expect(isEntryVisible(matchedWithDiff, "changes", "alice")).toBe(true);
    expect(isEntryVisible(diffNoMatch, "changes", "alice")).toBe(false);
  });
});

describe("visibleRepos", () => {
  it("narrows to repos with a search match in All access mode", () => {
    const alice = repo({ repo: "alice-repo", principals: [principal({ principal: { id: "u1", label: "Alice" } })] });
    const bob = repo({ repo: "bob-repo", principals: [principal({ principal: { id: "u2", label: "Bob" } })] });
    const p = project([alice, bob]);
    expect(visibleRepos(p, "all", "alice")).toEqual([alice]);
  });

  it("narrows to repos with a search match in Changes only mode", () => {
    const hotMatch = repo({
      repo: "hot-match",
      principals: [principal({ principal: { id: "u1", label: "Alice" }, diffStatus: { status: "Grant" } })],
    });
    const hotNoMatch = repo({
      repo: "hot-no-match",
      principals: [principal({ principal: { id: "u2", label: "Bob" }, diffStatus: { status: "Grant" } })],
    });
    const p = project([hotMatch, hotNoMatch]);
    expect(visibleRepos(p, "changes", "alice")).toEqual([hotMatch]);
  });

  it("excludes a repo with a search match but no diff in Changes only mode", () => {
    const coldMatch = repo({
      repo: "cold-match",
      principals: [principal({ principal: { id: "u1", label: "Alice" } })],
    });
    const p = project([coldMatch]);
    expect(visibleRepos(p, "changes", "alice")).toEqual([]);
  });

  it("excludes a repo with a diff but no search match when a query is set", () => {
    const hotNoMatch = repo({
      repo: "hot-no-match",
      principals: [principal({ principal: { id: "u2", label: "Bob" }, diffStatus: { status: "Grant" } })],
    });
    const p = project([hotNoMatch]);
    expect(visibleRepos(p, "changes", "alice")).toEqual([]);
  });
});

describe("viewCounts", () => {
  it("only counts entries belonging to repos passing both the mode and query predicates", () => {
    const aliceRepo = repo({
      repo: "alice-repo",
      principals: [
        principal({ principal: { id: "u1", label: "Alice" }, diffStatus: { status: "Grant" } }),
        principal({ principal: { id: "u2", label: "Zed" } }),
      ],
    });
    const bobRepo = repo({
      repo: "bob-repo",
      principals: [principal({ principal: { id: "u3", label: "Bob" }, diffStatus: { status: "Revoke" } })],
    });
    const tree = [project([aliceRepo, bobRepo])];

    // "all" mode: bobRepo is filtered out by the "alice" query, and within aliceRepo only the
    // matching "Alice" row counts — "Zed" doesn't match the query, so it's excluded too
    // (search narrows to matching rows, not every row in a repo that happens to match somewhere).
    expect(viewCounts(tree, "all", "alice")).toEqual({ live: 1, changed: 1, rows: 1 });

    // "changes" mode ANDs on top: aliceRepo is visible (has a match and a diff), only its
    // diffed row counts — the non-diffed "Zed" row is excluded by the mode filter.
    expect(viewCounts(tree, "changes", "alice")).toEqual({ live: 1, changed: 1, rows: 1 });

    // No query: both repos count, mirroring pre-search behavior. bobRepo's Revoke is "changed"
    // but not "live" (it no longer holds the permission); aliceRepo's two rows are both live.
    expect(viewCounts(tree, "all", "")).toEqual({ live: 2, changed: 2, rows: 3 });
  });
});
