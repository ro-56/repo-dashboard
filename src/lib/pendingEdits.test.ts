import { describe, expect, it } from "vitest";
import {
  applyErrorMessage,
  clearEdits,
  confirmRows,
  editKey,
  resultRows,
  settleApplied,
  stageEdit,
  toApplyPayload,
  undoEdit,
  type ApplyResult,
  type EditTarget,
  type PendingEdits,
  type StagedEdit,
} from "./pendingEdits";

function direct(id: string): EditTarget {
  return { type: "Direct", id };
}

function levelEdit(overrides: Partial<StagedEdit> = {}): StagedEdit {
  return {
    request: {
      scope: "Repo",
      target: direct("acct-1"),
      repoProject: "TEAM",
      repo: "repo-a",
      action: { type: "SetLevel", level: "Admin" },
    },
    principalLabel: "Ada",
    beforeLevel: "Write",
    ...overrides,
  };
}

function removeEdit(overrides: Partial<StagedEdit> = {}): StagedEdit {
  return {
    request: {
      scope: "Repo",
      target: direct("acct-2"),
      repoProject: "TEAM",
      repo: "repo-b",
      action: { type: "Remove" },
    },
    principalLabel: "Grace",
    beforeLevel: "Read",
    ...overrides,
  };
}

describe("editKey", () => {
  it("is stable for the same scope/target/repo and differs across any one of them", () => {
    const a = editKey("Repo", direct("acct-1"), "TEAM", "repo-a");
    const b = editKey("Repo", direct("acct-1"), "TEAM", "repo-a");
    expect(a).toBe(b);

    expect(editKey("Repo", direct("acct-2"), "TEAM", "repo-a")).not.toBe(a);
    expect(editKey("Repo", direct("acct-1"), "TEAM", "repo-b")).not.toBe(a);
    expect(editKey("Project", direct("acct-1"), "TEAM", "repo-a")).not.toBe(a);
  });
});

describe("stageEdit / undoEdit / clearEdits", () => {
  it("stages an edit under its derived key without mutating the input map", () => {
    const empty: PendingEdits = new Map();
    const edit = levelEdit();

    const staged = stageEdit(empty, edit);

    expect(empty.size).toBe(0);
    expect(staged.size).toBe(1);
    const key = editKey(edit.request.scope, edit.request.target, edit.request.repoProject, edit.request.repo);
    expect(staged.get(key)).toEqual(edit);
  });

  it("staging a second edit on the same slot replaces the first rather than accumulating", () => {
    const edit = levelEdit();
    const replacement = levelEdit({ request: { ...edit.request, action: { type: "Remove" } } });

    const pending = stageEdit(stageEdit(new Map(), edit), replacement);

    expect(pending.size).toBe(1);
    const key = editKey(edit.request.scope, edit.request.target, edit.request.repoProject, edit.request.repo);
    expect(pending.get(key)?.request.action).toEqual({ type: "Remove" });
  });

  it("undoEdit removes only the targeted key, leaving others staged", () => {
    const a = levelEdit();
    const b = removeEdit();
    const pending = stageEdit(stageEdit(new Map(), a), b);
    const keyA = editKey(a.request.scope, a.request.target, a.request.repoProject, a.request.repo);
    const keyB = editKey(b.request.scope, b.request.target, b.request.repoProject, b.request.repo);

    const next = undoEdit(pending, keyA);

    expect(next.has(keyA)).toBe(false);
    expect(next.has(keyB)).toBe(true);
    expect(pending.has(keyA)).toBe(true); // original untouched
  });

  it("clearEdits returns a fresh empty map", () => {
    expect(clearEdits().size).toBe(0);
  });
});

describe("confirmRows", () => {
  it("derives a sorted view model from staged edits, one row per edit", () => {
    const pending = stageEdit(stageEdit(new Map(), levelEdit()), removeEdit());

    const rows = confirmRows(pending);

    expect(rows).toHaveLength(2);
    // sorted by repo then principal label: repo-a before repo-b
    expect(rows[0]).toMatchObject({
      repo: "repo-a",
      principalLabel: "Ada",
      fromLevel: "write",
      toLevel: "admin",
    });
    expect(rows[1]).toMatchObject({
      repo: "repo-b",
      principalLabel: "Grace",
      fromLevel: "read",
      toLevel: "removed",
    });
  });

  it("carries a staged Group removal's cascadeCount straight through", () => {
    const groupRemoval = removeEdit({
      request: {
        scope: "Repo",
        target: { type: "Group", id: "platform-eng" },
        repoProject: "TEAM",
        repo: "repo-c",
        action: { type: "Remove" },
      },
      principalLabel: "Platform Eng",
      cascadeCount: 4,
    });
    const pending = stageEdit(new Map(), groupRemoval);

    const rows = confirmRows(pending);

    expect(rows[0].cascadeCount).toBe(4);
  });

  it("leaves cascadeCount undefined for edits that never set it", () => {
    const pending = stageEdit(new Map(), levelEdit());

    const rows = confirmRows(pending);

    expect(rows[0].cascadeCount).toBeUndefined();
  });
});

describe("toApplyPayload", () => {
  it("extracts just the wire requests, in no particular guaranteed row order", () => {
    const pending = stageEdit(stageEdit(new Map(), levelEdit()), removeEdit());

    const payload = toApplyPayload(pending);

    expect(payload).toHaveLength(2);
    expect(payload).toEqual(expect.arrayContaining([levelEdit().request, removeEdit().request]));
  });
});

describe("applyErrorMessage", () => {
  it("renders a human-readable string for each error type", () => {
    expect(applyErrorMessage({ type: "Unauthorized" })).toMatch(/scope/);
    expect(applyErrorMessage({ type: "RateLimited" })).toMatch(/rate limited/);
    expect(applyErrorMessage({ type: "Other", message: "already changed by someone else" })).toBe(
      "already changed by someone else",
    );
  });
});

describe("settleApplied", () => {
  it("removes successfully-applied edits and leaves failed ones staged for retry", () => {
    const a = levelEdit();
    const b = removeEdit();
    const pending = stageEdit(stageEdit(new Map(), a), b);
    const keyA = editKey(a.request.scope, a.request.target, a.request.repoProject, a.request.repo);
    const keyB = editKey(b.request.scope, b.request.target, b.request.repoProject, b.request.repo);

    const results: ApplyResult[] = [
      { request: a.request, outcome: { Ok: null } },
      { request: b.request, outcome: { Err: { type: "Other", message: "409 conflict" } } },
    ];

    const next = settleApplied(pending, results);

    expect(next.has(keyA)).toBe(false);
    expect(next.has(keyB)).toBe(true);
  });

  it("does not mutate the input map", () => {
    const a = levelEdit();
    const pending = stageEdit(new Map(), a);
    const results: ApplyResult[] = [{ request: a.request, outcome: { Ok: null } }];

    settleApplied(pending, results);

    expect(pending.size).toBe(1);
  });
});

describe("resultRows", () => {
  it("re-attaches principal label and before-level from the snapshot, per outcome", () => {
    const a = levelEdit();
    const b = removeEdit();
    const snapshot = stageEdit(stageEdit(new Map(), a), b);
    const results: ApplyResult[] = [
      { request: a.request, outcome: { Ok: null } },
      { request: b.request, outcome: { Err: { type: "Other", message: "409 conflict" } } },
    ];

    const rows = resultRows(snapshot, results);

    expect(rows).toEqual([
      {
        key: editKey(a.request.scope, a.request.target, a.request.repoProject, a.request.repo),
        repoProject: "TEAM",
        repo: "repo-a",
        principalLabel: "Ada",
        fromLevel: "write",
        toLevel: "admin",
        ok: true,
        errorMessage: undefined,
      },
      {
        key: editKey(b.request.scope, b.request.target, b.request.repoProject, b.request.repo),
        repoProject: "TEAM",
        repo: "repo-b",
        principalLabel: "Grace",
        fromLevel: "read",
        toLevel: "removed",
        ok: false,
        errorMessage: "409 conflict",
      },
    ]);
  });

  it("falls back to blank display context when the result has no matching staged edit", () => {
    const orphanRequest = removeEdit().request;
    const results: ApplyResult[] = [{ request: orphanRequest, outcome: { Ok: null } }];

    const rows = resultRows(new Map(), results);

    expect(rows[0].principalLabel).toBe("");
    expect(rows[0].fromLevel).toBe("");
    expect(rows[0].ok).toBe(true);
  });
});
