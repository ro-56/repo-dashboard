import { describe, expect, it } from "vitest";
import { escapeCsvField, exportFilename, rosterToCsv } from "./rosterExport";
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

function tree(repos: RepoNode[], repoProject = "TEAM"): RosterTree {
  const project: ProjectNode = { repoProject, repos };
  return [project];
}

describe("escapeCsvField", () => {
  it("passes through plain fields unquoted", () => {
    expect(escapeCsvField("alice")).toBe("alice");
  });

  it("quotes a field containing a comma", () => {
    expect(escapeCsvField("smith, alice")).toBe('"smith, alice"');
  });

  it("quotes and doubles embedded quotes", () => {
    expect(escapeCsvField('the "alice" user')).toBe('"the ""alice"" user"');
  });

  it("quotes a field containing a newline", () => {
    expect(escapeCsvField("line1\nline2")).toBe('"line1\nline2"');
  });
});

describe("rosterToCsv", () => {
  it("emits the header row followed by one row per principal", () => {
    const t = tree([repo({ principals: [principal()] })]);
    const csv = rosterToCsv(t, "2026-08-01T19:19:10Z");
    const lines = csv.split("\n");
    expect(lines[0]).toBe("repo_project,repo,username,display_name,access_type,scope,permission,run_at");
    expect(lines[1]).toBe("TEAM,widget,u1,alice,direct,repo,read,2026-08-01T19:19:10Z");
    expect(lines).toHaveLength(2);
  });

  it("renders Direct, Group, and Member access types distinctly", () => {
    const t = tree([
      repo({
        principals: [
          principal({ accessType: { type: "Direct" } }),
          principal({ principal: { id: "g1", label: "devs" }, accessType: { type: "Group" } }),
          principal({
            principal: { id: "u2", label: "bob" },
            accessType: { type: "Member", group_id: "g1" },
            scope: "Repo",
          }),
        ],
      }),
    ]);
    const rows = rosterToCsv(t, "2026-08-01T19:19:10Z").split("\n").slice(1);
    expect(rows[0]).toContain(",direct,");
    expect(rows[1]).toContain(",group,");
    expect(rows[2]).toContain(",member:g1,");
  });

  it("displays CreateRepo as create-repo, never collapsed to admin", () => {
    const t = tree([repo({ principals: [principal({ permission: "CreateRepo" })] })]);
    const rows = rosterToCsv(t, "2026-08-01T19:19:10Z").split("\n").slice(1);
    expect(rows[0]).toContain(",create-repo,");
  });

  it("keeps a Repo-scope and Project-scope row for the same principal on the same repo separate", () => {
    const t = tree([
      repo({
        principals: [
          principal({ scope: "Repo", permission: "Read" }),
          principal({ scope: "Project", permission: "Admin" }),
        ],
      }),
    ]);
    const rows = rosterToCsv(t, "2026-08-01T19:19:10Z").split("\n").slice(1);
    expect(rows).toHaveLength(2);
    expect(rows[0]).toContain(",repo,read,");
    expect(rows[1]).toContain(",project,admin,");
  });

  it("escapes values containing a comma, quote, or newline", () => {
    const t = tree([
      repo({
        principals: [
          principal({ principal: { id: "u3", label: 'o"brien, "the" boss' } }),
        ],
      }),
    ]);
    const rows = rosterToCsv(t, "2026-08-01T19:19:10Z").split("\n").slice(1);
    expect(rows[0]).toContain('"o""brien, ""the"" boss"');
  });

  it("is diff-free even when the input tree carries non-None diffStatus values", () => {
    const t = tree([
      repo({
        principals: [
          principal({ diffStatus: { status: "Grant" } }),
          principal({
            principal: { id: "u2", label: "bob" },
            diffStatus: { status: "LevelChange", from: "Read", to: "Admin", kind: "Escalation" },
          }),
        ],
      }),
    ]);
    const csv = rosterToCsv(t, "2026-08-01T19:19:10Z");
    expect(csv).not.toMatch(/Grant|Revoke|Escalation|Demotion|LevelChange/);
  });
});

describe("exportFilename", () => {
  it("combines workspace and a filename-safe run timestamp", () => {
    expect(exportFilename("acme", "2026-08-01T19:19:10Z")).toBe("roster_acme_2026-08-01T19-19-10Z.csv");
  });
});
