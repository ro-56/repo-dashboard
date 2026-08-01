import { describe, expect, it } from "vitest";
import {
  compareEntries,
  deriveRow,
  editTargetForEntry,
  hasDiff,
  isUnresolvedGroup,
  pendingRowView,
  rowMenu,
  sortedPrincipals,
  sourceLabel,
  sourceTooltip,
} from "./rosterRow";
import type { AccessType, PrincipalEntry } from "./roster";
import type { StagedEdit } from "./pendingEdits";

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

  it("previews a pending level change: effective level, transition, and pending tag", () => {
    const view = deriveRow(entry({ permission: "Write", diffStatus: { status: "None" } }), {
      kind: "level",
      beforeLevel: "Write",
      afterLevel: "Admin",
    });
    expect(view.levelWord).toBe("admin");
    expect(view.levelClass).toBe("lvl-admin");
    expect(view.state).toBe("modified");
    expect(view.sigil).toBe("~");
    expect(view.struck).toBe(false);
    expect(view.from).toBe("write");
    expect(view.to).toBe("admin");
    expect(view.tags).toEqual([{ label: "pending · unsaved", kind: "pending-level" }]);
  });

  it("previews a pending removal: struck through, transitions to 'removed'", () => {
    const view = deriveRow(entry({ permission: "Read", diffStatus: { status: "None" } }), {
      kind: "remove",
      beforeLevel: "Read",
    });
    expect(view.state).toBe("modified");
    expect(view.sigil).toBe("−");
    expect(view.struck).toBe(true);
    expect(view.from).toBe("read");
    expect(view.to).toBe("removed");
    expect(view.tags).toEqual([{ label: "pending removal", kind: "pending-remove" }]);
  });

  it("keeps an existing escalation tag alongside the pending tag, pending tag first", () => {
    const view = deriveRow(
      entry({ diffStatus: { status: "LevelChange", from: "Read", to: "Write", kind: "Escalation" } }),
      { kind: "level", beforeLevel: "Write", afterLevel: "Admin" },
    );
    expect(view.tags).toEqual([
      { label: "pending · unsaved", kind: "pending-level" },
      { label: "escalation", kind: "esc" },
    ]);
  });

  it("a row with no pending edit renders identically to omitting the argument entirely", () => {
    const e = entry({ diffStatus: { status: "Grant" }, permission: "Read" });
    expect(deriveRow(e, undefined)).toEqual(deriveRow(e));
  });
});

describe("pendingRowView", () => {
  function staged(overrides: Partial<StagedEdit> = {}): StagedEdit {
    return {
      request: {
        scope: "Repo",
        target: { type: "Direct", id: "acct-1" },
        repoProject: "TEAM",
        repo: "repo-a",
        action: { type: "SetLevel", level: "Admin" },
      },
      principalLabel: "Ada",
      beforeLevel: "Write",
      ...overrides,
    };
  }

  it("is undefined when nothing is staged", () => {
    expect(pendingRowView(undefined)).toBeUndefined();
  });

  it("maps a SetLevel action to a 'level' view with before/after", () => {
    expect(pendingRowView(staged())).toEqual({ kind: "level", beforeLevel: "Write", afterLevel: "Admin" });
  });

  it("maps a Remove action to a 'remove' view with only the before level", () => {
    expect(pendingRowView(staged({ request: { ...staged().request, action: { type: "Remove" } } }))).toEqual({
      kind: "remove",
      beforeLevel: "Write",
    });
  });
});

describe("editTargetForEntry", () => {
  it("targets Direct entries at Repo scope", () => {
    const e = entry({ accessType: { type: "Direct" }, scope: "Repo" });
    expect(editTargetForEntry(e)).toEqual({ type: "Direct", id: "u1" });
  });

  it("is null for Group entries, Member entries, and Project-scope entries", () => {
    expect(editTargetForEntry(entry({ accessType: { type: "Group" }, scope: "Repo" }))).toBeNull();
    expect(
      editTargetForEntry(entry({ accessType: { type: "Member", group_id: "secops" }, scope: "Repo" })),
    ).toBeNull();
    expect(editTargetForEntry(entry({ accessType: { type: "Direct" }, scope: "Project" }))).toBeNull();
  });
});

describe("rowMenu", () => {
  it("is null for entries editTargetForEntry rejects", () => {
    expect(rowMenu(entry({ accessType: { type: "Group" } }), undefined)).toBeNull();
  });

  it("offers Read/Write/Admin, highlighting the entry's current level when nothing is staged", () => {
    const menu = rowMenu(entry({ accessType: { type: "Direct" }, scope: "Repo", permission: "Write" }), undefined);
    expect(menu?.effectiveLevel).toBe("Write");
    expect(menu?.levelOptions).toEqual([
      { level: "Read", active: false },
      { level: "Write", active: true },
      { level: "Admin", active: false },
    ]);
  });

  it("highlights the staged level instead of the entry's true current level when one is pending", () => {
    const e = entry({ accessType: { type: "Direct" }, scope: "Repo", permission: "Read" });
    const edit: StagedEdit = {
      request: {
        scope: "Repo",
        target: { type: "Direct", id: e.principal.id },
        repoProject: "TEAM",
        repo: "repo-a",
        action: { type: "SetLevel", level: "Admin" },
      },
      principalLabel: e.principal.label,
      beforeLevel: "Read",
    };
    const menu = rowMenu(e, edit);
    expect(menu?.effectiveLevel).toBe("Admin");
    expect(menu?.levelOptions.find((o) => o.level === "Admin")?.active).toBe(true);
  });

  it("treats create-repo as admin-tier for the effective level, same as elsewhere", () => {
    const menu = rowMenu(entry({ accessType: { type: "Direct" }, scope: "Repo", permission: "CreateRepo" }), undefined);
    expect(menu?.effectiveLevel).toBe("Admin");
  });
});
