// Local staging area for permission edits (PD-59/PD-60, ADR-0022) — nothing here calls
// Bitbucket. Edits accumulate keyed by scope+target+repo across the whole roster tree (not
// scoped per repo card) until a single global Apply sends the batch to the `apply_pending_edits`
// Tauri command. Mirrors src-tauri/src/apply.rs's wire shape by hand — no shared schema yet.

import type { GrantScope, Permission } from "./roster";

export type EditTarget = { type: "Direct"; id: string } | { type: "Group"; id: string };

// The level picker only ever offers these three (ADR-0021) — CreateRepo is Project-scope-only
// in the read model and was never meant to be user-settable here.
export type EditableLevel = "Read" | "Write" | "Admin";

export type EditAction = { type: "SetLevel"; level: EditableLevel } | { type: "Remove" };

export interface PendingEditRequest {
  scope: GrantScope;
  target: EditTarget;
  repoProject: string;
  repo: string;
  action: EditAction;
}

// Display context alongside the wire request — `apply_pending_edits` never needs the principal's
// label or the pre-edit level, but the row's "pending" tag and the confirm dialog both do.
export interface StagedEdit {
  request: PendingEditRequest;
  principalLabel: string;
  beforeLevel: Permission;
  // How many Member grants derive from this Group grant (rosterRow.ts's `cascadeMemberCount`,
  // ADR-0022/PD-62) — only ever set for a Group `Remove` action; `null` when membership is
  // unresolvable (CONTEXT.md), `undefined` for anything else (Direct edits, Group level changes).
  cascadeCount?: number | null;
}

export type PendingEdits = Map<string, StagedEdit>;

/** Identifies a staged edit's slot regardless of what's currently staged there — a second edit
 * on the same grant replaces the first rather than accumulating two. */
export function editKey(scope: GrantScope, target: EditTarget, repoProject: string, repo: string): string {
  return [repoProject, repo, scope, target.type, target.id].join("::");
}

export function stageEdit(pending: PendingEdits, edit: StagedEdit): PendingEdits {
  const key = editKey(edit.request.scope, edit.request.target, edit.request.repoProject, edit.request.repo);
  const next = new Map(pending);
  next.set(key, edit);
  return next;
}

export function undoEdit(pending: PendingEdits, key: string): PendingEdits {
  const next = new Map(pending);
  next.delete(key);
  return next;
}

export function clearEdits(): PendingEdits {
  return new Map();
}

export interface ConfirmRow {
  key: string;
  repoProject: string;
  repo: string;
  principalLabel: string;
  fromLevel: string;
  toLevel: string; // "removed" for a Remove action
  // Carried straight through from the staged edit (PD-62) — see `StagedEdit.cascadeCount`.
  cascadeCount?: number | null;
}

/** The confirm dialog's view model — every staged change, sorted for stable, scannable review
 * rather than staging order. */
export function confirmRows(pending: PendingEdits): ConfirmRow[] {
  return [...pending.entries()]
    .map(([key, edit]) => ({
      key,
      repoProject: edit.request.repoProject,
      repo: edit.request.repo,
      principalLabel: edit.principalLabel,
      fromLevel: edit.beforeLevel.toLowerCase(),
      toLevel: edit.request.action.type === "SetLevel" ? edit.request.action.level.toLowerCase() : "removed",
      cascadeCount: edit.cascadeCount,
    }))
    .sort((a, b) => a.repo.localeCompare(b.repo) || a.principalLabel.localeCompare(b.principalLabel));
}

export function toApplyPayload(pending: PendingEdits): PendingEditRequest[] {
  return [...pending.values()].map((edit) => edit.request);
}

// ---- apply_pending_edits response shape (mirrors src-tauri/src/apply.rs's Serialize impls) ----

export type ApplyError = { type: "Unauthorized" } | { type: "RateLimited" } | { type: "Other"; message: string };

// Rust's derived Result<(), ApplyError> serializes as the externally-tagged { Ok: null } /
// { Err: ApplyError } shape, not a discriminated union of our own choosing.
export interface ApplyResult {
  request: PendingEditRequest;
  outcome: { Ok: null } | { Err: ApplyError };
}

export function applyErrorMessage(error: ApplyError): string {
  switch (error.type) {
    case "Unauthorized":
      return "credential lacks the required scope";
    case "RateLimited":
      return "rate limited — retry";
    case "Other":
      return error.message;
  }
}

/** Drops every successfully-applied edit from `pending`; a failed item stays staged for retry
 * (PD-60 acceptance criteria) — matched back to its slot by the same key it was staged under,
 * not by array order, so this stays correct even if a future caller reorders results. */
export function settleApplied(pending: PendingEdits, results: ApplyResult[]): PendingEdits {
  const next = new Map(pending);
  for (const result of results) {
    if ("Ok" in result.outcome) {
      const { scope, target, repoProject, repo } = result.request;
      next.delete(editKey(scope, target, repoProject, repo));
    }
  }
  return next;
}

export interface ResultRow extends ConfirmRow {
  ok: boolean;
  errorMessage?: string;
}

/** The successful subset of a batch's `ApplyResult`s — the only ones Refresh (PD-70) ever
 * touches, both to decide whether to offer it and to build the exact payload it sends. */
export function successfulResults(results: ApplyResult[]): ApplyResult[] {
  return results.filter((result) => "Ok" in result.outcome);
}

/** Whether "Refresh affected" should be offered for this batch: at least one edit succeeded,
 * regardless of scope mix (PD-71 — Repo-scope and Project-scope targets are both refreshable,
 * so a batch mixing them, or one entirely Project-scope, no longer falls back to the re-run
 * prompt). Only an all-failed batch is a no-op with nothing to offer. */
export function canRefresh(results: ApplyResult[]): boolean {
  return successfulResults(results).length > 0;
}

/** The confirm dialog's post-apply results view: one row per `ApplyResult`, re-attaching the
 * display context (principal label, before-level) from the snapshot of `pending` the batch was
 * built from, since the wire-level `ApplyResult` only carries what `apply_pending_edits` itself
 * needed. Matched by the same key `settleApplied` uses, not array order. */
export function resultRows(snapshot: PendingEdits, results: ApplyResult[]): ResultRow[] {
  return results.map((result) => {
    const { scope, target, repoProject, repo, action } = result.request;
    const key = editKey(scope, target, repoProject, repo);
    const staged = snapshot.get(key);
    const ok = "Ok" in result.outcome;
    return {
      key,
      repoProject,
      repo,
      principalLabel: staged?.principalLabel ?? "",
      fromLevel: staged ? staged.beforeLevel.toLowerCase() : "",
      toLevel: action.type === "SetLevel" ? action.level.toLowerCase() : "removed",
      ok,
      errorMessage: ok ? undefined : applyErrorMessage((result.outcome as { Err: ApplyError }).Err),
    };
  });
}
