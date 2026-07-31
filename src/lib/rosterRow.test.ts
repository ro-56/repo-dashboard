import { describe, expect, it } from "vitest";
import {
  compareEntries,
  deriveRow,
  hasDiff,
  isUnresolvedGroup,
  sortedPrincipals,
  sourceLabel,
  sourceTooltip,
} from "./rosterRow";
import type { AccessType, PrincipalEntry } from "./roster";

function entry(overrides: Partial<PrincipalEntry> = {}): PrincipalEntry {
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

describe("sourceLabel", () => {
  const cases: [AccessType, string][] = [
    [{ type: "Direct" }, "direct"],
    [{ type: "Group" }, "group"],
    [{ type: "Member", group_id: "secops" }, "grp:secops"],
  ];

  it.each(cases)("renders %o Repo-scoped as %s with no suffix", (accessType, label) => {
    expect(sourceLabel(accessType, "Repo")).toBe(label);
  });

  it.each(cases)("prepends '↳ ' to %o when Project-scoped", (accessType, label) => {
    expect(sourceLabel(accessType, "Project")).toBe(`↳ ${label}`);
  });
});

describe("sourceTooltip", () => {
  const cases: [AccessType, string][] = [
    [{ type: "Direct" }, "direct"],
    [{ type: "Group" }, "group"],
    [{ type: "Member", group_id: "secops" }, "grp:secops"],
  ];

  it.each(cases)("echoes the visible label for %o Repo-scoped entries", (accessType, label) => {
    expect(sourceTooltip(accessType, "Repo")).toBe(label);
  });

  it.each(cases)("reads as an explanatory string for %o Project-scoped entries", (accessType) => {
    expect(sourceTooltip(accessType, "Project")).toBe("Project-level grant");
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

  it("ranks create-repo at the same tier as admin, ahead of write", () => {
    const write = entry({ principal: { id: "u1", label: "bob" }, permission: "Write" });
    const createRepo = entry({ principal: { id: "u2", label: "carol" }, permission: "CreateRepo" });
    const admin = entry({ principal: { id: "u3", label: "alice" }, permission: "Admin" });

    expect(sortedPrincipals([write, createRepo, admin])).toEqual([admin, createRepo, write]);
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

  it("renders a create-repo grant with admin-tier styling but its own label", () => {
    const view = deriveRow(entry({ diffStatus: { status: "None" }, permission: "CreateRepo" }));
    expect(view.levelClass).toBe("lvl-admin");
    expect(view.levelWord).toBe("create-repo");
  });

  it("renders a new create-repo grant as added, to its own label", () => {
    const view = deriveRow(entry({ diffStatus: { status: "Grant" }, permission: "CreateRepo" }));
    expect(view.state).toBe("added");
    expect(view.levelClass).toBe("lvl-admin");
    expect(view.to).toBe("create-repo");
  });

  it("tags an escalation into create-repo the same as an escalation into admin", () => {
    const view = deriveRow(
      entry({
        diffStatus: { status: "LevelChange", from: "Write", to: "CreateRepo", kind: "Escalation" },
      }),
    );
    expect(view.isEscalation).toBe(true);
    expect(view.to).toBe("create-repo");
    expect(view.tags).toEqual([{ label: "escalation → admin", kind: "esc" }]);
  });

  it("adds a 'members unresolved' tag only for an unresolved Group's own row", () => {
    const view = deriveRow(
      entry({ accessType: { type: "Group" }, membersResolved: false, diffStatus: { status: "None" } }),
    );
    expect(view.tags).toEqual([{ label: "members unresolved", kind: "unresolved" }]);
  });
});
