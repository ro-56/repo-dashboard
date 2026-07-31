import { describe, expect, it } from "vitest";
import {
  compareEntries,
  deriveRow,
  hasDiff,
  isUnresolvedGroup,
  sortedPrincipals,
  sourceLabel,
} from "./rosterRow";
import type { AccessType, PrincipalEntry } from "./roster";

function entry(overrides: Partial<PrincipalEntry> = {}): PrincipalEntry {
  return {
    principal: { id: "u1", label: "alice" },
    accessType: { type: "Direct" },
    scope: "Repo",
    permission: "Read",
    diffStatus: { status: "None" },
    workspaceState: "present",
    membersResolved: null,
    ...overrides,
  };
}

describe("sourceLabel", () => {
  const cases: [AccessType, string][] = [
    [{ type: "Direct" }, "direct"],
    [{ type: "Group" }, "group"],
    [{ type: "Member", group_id: "secops" }, "grp:secops"],
  ];

  it.each(cases)("renders %o Repo-scoped as %s with no suffix", (accessType, label) => {
    expect(sourceLabel(accessType, "Repo")).toBe(label);
  });

  it.each(cases)("appends ' · project' to %o when Project-scoped", (accessType, label) => {
    expect(sourceLabel(accessType, "Project")).toBe(`${label} · project`);
  });
});

describe("isUnresolvedGroup", () => {
  it("is true only for a Group's own entry with membersResolved false", () => {
    expect(isUnresolvedGroup(entry({ accessType: { type: "Group" }, membersResolved: false }))).toBe(
      true,
    );
  });

  it("is false for a resolved Group and for non-Group entries", () => {
    expect(isUnresolvedGroup(entry({ accessType: { type: "Group" }, membersResolved: true }))).toBe(
      false,
    );
    expect(
      isUnresolvedGroup(
        entry({ accessType: { type: "Member", group_id: "secops" }, membersResolved: null }),
      ),
    ).toBe(false);
  });
});

describe("hasDiff", () => {
  it("is false for None and true for any diff status", () => {
    expect(hasDiff(entry({ diffStatus: { status: "None" } }))).toBe(false);
    expect(hasDiff(entry({ diffStatus: { status: "Grant" } }))).toBe(true);
  });
});

describe("compareEntries / sortedPrincipals", () => {
  it("orders admin before write before read, then by label, then direct before group", () => {
    const read = entry({ principal: { id: "u1", label: "zeta" }, permission: "Read" });
    const admin = entry({ principal: { id: "u2", label: "alice" }, permission: "Admin" });
    const write = entry({ principal: { id: "u3", label: "bob" }, permission: "Write" });

    expect(sortedPrincipals([read, write, admin])).toEqual([admin, write, read]);
  });

  it("breaks ties on same label by source rank: direct before group before member", () => {
    const direct = entry({ accessType: { type: "Direct" } });
    const group = entry({ accessType: { type: "Group" } });
    const member = entry({ accessType: { type: "Member", group_id: "secops" } });

    expect(compareEntries(direct, group)).toBeLessThan(0);
    expect(compareEntries(group, member)).toBeLessThan(0);
  });
});

describe("deriveRow", () => {
  it("renders a None row as unchanged with no transition", () => {
    const view = deriveRow(entry({ diffStatus: { status: "None" }, permission: "Write" }));
    expect(view.state).toBe("same");
    expect(view.showTransition).toBe(false);
    expect(view.struck).toBe(false);
    expect(view.tags).toEqual([]);
  });

  it("renders a Grant row as added, from em-dash to the level", () => {
    const view = deriveRow(entry({ diffStatus: { status: "Grant" }, permission: "Admin" }));
    expect(view.state).toBe("added");
    expect(view.from).toBe("—");
    expect(view.to).toBe("admin");
  });

  it("renders a Revoke row as removed and struck through", () => {
    const view = deriveRow(entry({ diffStatus: { status: "Revoke" }, permission: "Write" }));
    expect(view.state).toBe("removed");
    expect(view.struck).toBe(true);
    expect(view.to).toBe("—");
  });

  it("renders ↑ for an escalation LevelChange and ~ for a plain one", () => {
    const escalation = deriveRow(
      entry({ diffStatus: { status: "LevelChange", from: "Read", to: "Write", kind: "Escalation" } }),
    );
    expect(escalation.sigil).toBe("↑");

    const demotion = deriveRow(
      entry({ diffStatus: { status: "LevelChange", from: "Admin", to: "Read", kind: "Demotion" } }),
    );
    expect(demotion.sigil).toBe("~");
  });

  it("tags an Escalation LevelChange, and calls out admin specifically", () => {
    const toAdmin = deriveRow(
      entry({ diffStatus: { status: "LevelChange", from: "Write", to: "Admin", kind: "Escalation" } }),
    );
    expect(toAdmin.isEscalation).toBe(true);
    expect(toAdmin.tags).toEqual([{ label: "escalation → admin", kind: "esc" }]);

    const toWrite = deriveRow(
      entry({ diffStatus: { status: "LevelChange", from: "Read", to: "Write", kind: "Escalation" } }),
    );
    expect(toWrite.tags).toEqual([{ label: "escalation", kind: "esc" }]);
  });

  it("does not tag a Demotion LevelChange", () => {
    const view = deriveRow(
      entry({ diffStatus: { status: "LevelChange", from: "Admin", to: "Read", kind: "Demotion" } }),
    );
    expect(view.isEscalation).toBe(false);
    expect(view.tags).toEqual([]);
  });

  it("adds a 'members unresolved' tag only for an unresolved Group's own row", () => {
    const view = deriveRow(
      entry({ accessType: { type: "Group" }, membersResolved: false, diffStatus: { status: "None" } }),
    );
    expect(view.tags).toEqual([{ label: "members unresolved", kind: "unresolved" }]);
  });
});
