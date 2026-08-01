// Row-level presentation logic for the roster grid (ADR-0005, ADR-0006, ADR-0007).
// Pure functions only — markup and colour live in RosterRow.svelte's scoped styles,
// keyed off the `state`/`levelClass` strings returned here, not inline style strings.

import type { EditableLevel, EditTarget, StagedEdit } from "./pendingEdits";
import type { AccessType, GrantScope, Permission, PrincipalEntry } from "./roster";

export type RowState = "same" | "added" | "removed" | "modified";
export type LevelClass = "lvl-admin" | "lvl-write" | "lvl-read";

type CoreLevel = "Read" | "Write" | "Admin";

/** Collapses `CreateRepo` onto `Admin`; every other variant passes through unchanged. Mirrors
 * `Permission::admin_light` on the Rust side (ADR-0024) — used only where code computes rank
 * or style tier, never for display (see levelWord, the display-side counterpart). */
function adminLight(permission: Permission): CoreLevel {
  return permission === "CreateRepo" ? "Admin" : permission;
}

const RANK: Record<CoreLevel, number> = { Read: 1, Write: 2, Admin: 3 };
const SOURCE_RANK: Record<AccessType["type"], number> = { Direct: 0, Group: 1, Member: 2 };
const LEVEL_CLASS: Record<CoreLevel, LevelClass> = { Admin: "lvl-admin", Write: "lvl-write", Read: "lvl-read" };

/** A Group's own entry whose membership could not be resolved (CONTEXT.md), backing the
 * row's own tag (below). */
export function isUnresolvedGroup(entry: PrincipalEntry): boolean {
  return entry.accessType.type === "Group" && entry.membersResolved === false;
}

/** How many `Member` entries in `candidates` derive from `entry`'s Group grant — the cascade
 * warning for a staged Group removal (ADR-0022, PD-62): those Member rows disappear along with
 * it, since a Member's access is computed from its group's own grant rather than stored
 * separately. `null` when unavailable rather than zero: `entry` isn't a Group at all, or its
 * membership is unresolvable (CONTEXT.md's Unresolvable membership) — in the latter case there's
 * no way to know how many members would be affected, so the count is omitted rather than
 * guessed. `candidates` is the caller's job to scope correctly: this repo's own principals for a
 * Repo-scope group, every repo's principals in the owning Project for a Project-scope one (a
 * Project-level group's Member rows are duplicated per repo it cascades to, ADR-0012). */
export function cascadeMemberCount(entry: PrincipalEntry, candidates: PrincipalEntry[]): number | null {
  if (entry.accessType.type !== "Group" || entry.membersResolved === false) return null;
  return candidates.filter(
    (p) => p.accessType.type === "Member" && p.accessType.group_id === entry.principal.id && p.scope === entry.scope,
  ).length;
}

/** Formats `cascadeMemberCount`'s result for display — shared by the row menu and the confirm
 * dialog so both ever say exactly the same thing about the same staged removal. `null` in,
 * `null` out: nothing renders when the count is unavailable. */
export function cascadeNote(count: number | null | undefined): string | null {
  if (count === null || count === undefined) return null;
  return `removing this also drops access for ${count} ${count === 1 ? "member" : "members"}`;
}

/** A row carrying a diff marker (Grant/Revoke/LevelChange) — shared by the Changes only tab's
 * row-level filter and its "N changed" count (filterBar.ts) so neither can drift from the other. */
export function hasDiff(entry: PrincipalEntry): boolean {
  return entry.diffStatus.status !== "None";
}

/** `direct`, `group`, or `grp:<name>`, plus a `↳ ` prefix for Project-level grants
 * (ADR-0020) — never spills into the notes column. */
export function sourceLabel(accessType: AccessType, scope: GrantScope): string {
  const base = (() => {
    switch (accessType.type) {
      case "Direct":
        return "direct";
      case "Group":
        return "group";
      case "Member":
        return `grp:${accessType.group_id}`;
    }
  })();
  return scope === "Project" ? `↳ ${base}` : base;
}

/** The `.source` cell's `title` tooltip: an explanatory string for Project-level grants
 * (ADR-0020), since the visible `↳` prefix no longer needs truncation insurance; unchanged
 * echo of the visible label for Repo-scoped grants. */
export function sourceTooltip(accessType: AccessType, scope: GrantScope): string {
  return scope === "Project" ? "Project-level grant" : sourceLabel(accessType, scope);
}

/** Roster ordering: admin → write → read, then principal label, then source (direct before group). */
export function compareEntries(a: PrincipalEntry, b: PrincipalEntry): number {
  return (
    RANK[adminLight(b.permission)] - RANK[adminLight(a.permission)] ||
    a.principal.label.localeCompare(b.principal.label) ||
    SOURCE_RANK[a.accessType.type] - SOURCE_RANK[b.accessType.type]
  );
}

export function sortedPrincipals(entries: PrincipalEntry[]): PrincipalEntry[] {
  return [...entries].sort(compareEntries);
}

export interface RosterRowTag {
  label: string;
  kind: "esc" | "unresolved" | "pending-level" | "pending-remove";
}

/** A staged edit's effect on how a row displays, decoupled from `StagedEdit`'s wire shape so
 * `deriveRow` only ever reasons about display-relevant fields. */
export interface PendingRowView {
  kind: "level" | "remove";
  beforeLevel: Permission;
  afterLevel?: Permission;
}

/** Adapts a `pendingEdits.ts` `StagedEdit` into `deriveRow`'s pending-display input. `undefined`
 * in, `undefined` out — a row with nothing staged renders exactly as before this feature. */
export function pendingRowView(staged: StagedEdit | undefined): PendingRowView | undefined {
  if (!staged) return undefined;
  if (staged.request.action.type === "SetLevel") {
    return { kind: "level", beforeLevel: staged.beforeLevel, afterLevel: staged.request.action.level };
  }
  return { kind: "remove", beforeLevel: staged.beforeLevel };
}

export interface RosterRowView {
  state: RowState;
  sigil: string;
  levelWord: string;
  levelClass: LevelClass;
  struck: boolean;
  isEscalation: boolean;
  showTransition: boolean;
  from: string;
  to: string;
  tags: RosterRowTag[];
}

const EDITABLE_LEVELS: EditableLevel[] = ["Read", "Write", "Admin"];

/** The edit target a row's menu would stage against, or `null` when this entry can't be edited
 * at all — Member rows get no menu at either scope, permanently (ADR-0021: a member's access is
 * derived from their group's own grant, and Bitbucket has no endpoint to set it independently).
 * Direct (PD-60/PD-61) and Group (PD-62) entries are both editable at Repo and Project scope. */
export function editTargetForEntry(entry: PrincipalEntry): EditTarget | null {
  switch (entry.accessType.type) {
    case "Direct":
      return { type: "Direct", id: entry.principal.id };
    case "Group":
      return { type: "Group", id: entry.principal.id };
    case "Member":
      return null;
  }
}

export interface LevelMenuOption {
  level: EditableLevel;
  active: boolean;
}

export interface RowMenu {
  levelOptions: LevelMenuOption[];
  effectiveLevel: EditableLevel;
  /** The cascade note for a Project-scope grant — `null` at Repo scope. PD-61 calls for reusing
   * whatever cascade note is "already shown elsewhere in the dashboard" rather than introducing
   * new note-computation logic ("no new note needed") — the only such note that exists today is
   * `sourceTooltip`'s "Project-level grant" string (ADR-0020's `.source`-cell tooltip), so that's
   * what's reused here rather than a newly-computed repo-count string. */
  scopeNote: string | null;
  /** How many Member grants would disappear if this Group grant were removed (PD-62) — `null`
   * for Direct entries and for a Group whose membership is unresolvable. See
   * `cascadeMemberCount`. */
  cascadeCount: number | null;
}

/** Builds the row menu's Read/Write/Admin options, highlighting whichever level is already
 * staged (if any) rather than the row's true current level — matching the reference design's
 * `effLevel`, so reopening the menu after staging a change shows the choice just made. Returns
 * `null` for anything `editTargetForEntry` won't produce a target for. `candidates` feeds
 * `cascadeMemberCount` for Group entries — irrelevant, and safe to omit, for Direct ones. */
export function rowMenu(entry: PrincipalEntry, staged: StagedEdit | undefined, candidates: PrincipalEntry[] = []): RowMenu | null {
  if (!editTargetForEntry(entry)) return null;
  const effectiveLevel: EditableLevel =
    staged?.request.action.type === "SetLevel" ? staged.request.action.level : (adminLight(entry.permission) as EditableLevel);
  return {
    levelOptions: EDITABLE_LEVELS.map((level) => ({ level, active: level === effectiveLevel })),
    effectiveLevel,
    scopeNote: entry.scope === "Project" ? sourceTooltip(entry.accessType, entry.scope) : null,
    cascadeCount: cascadeMemberCount(entry, candidates),
  };
}

function levelWord(permission: Permission): string {
  return permission === "CreateRepo" ? "create-repo" : permission.toLowerCase();
}

export function deriveRow(entry: PrincipalEntry, pending?: PendingRowView): RosterRowView {
  const status = entry.diffStatus;
  // A pending level-change previews its target level immediately (matching the reference
  // design's `effLevel`) — the meter/level-word reflect what Apply would set, not what's
  // currently live, since that's the whole point of showing it as "pending".
  const effPermission = pending?.kind === "level" && pending.afterLevel !== undefined ? pending.afterLevel : entry.permission;
  const levelClass = LEVEL_CLASS[adminLight(effPermission)];
  const base = {
    levelWord: levelWord(effPermission),
    levelClass,
  };

  const view: RosterRowView = (() => {
    switch (status.status) {
      case "None":
        return {
          ...base,
          state: "same",
          sigil: "·",
          struck: false,
          isEscalation: false,
          showTransition: false,
          from: "",
          to: "",
          tags: [],
        };
      case "Grant":
        return {
          ...base,
          state: "added",
          sigil: "+",
          struck: false,
          isEscalation: false,
          showTransition: true,
          from: "—",
          to: base.levelWord,
          tags: [],
        };
      case "Revoke":
        return {
          ...base,
          state: "removed",
          sigil: "−",
          struck: true,
          isEscalation: false,
          showTransition: true,
          from: base.levelWord,
          to: "—",
          tags: [],
        };
      case "LevelChange": {
        const isEscalation = status.kind === "Escalation";
        const tags: RosterRowTag[] = isEscalation
          ? [{ label: adminLight(status.to) === "Admin" ? "escalation → admin" : "escalation", kind: "esc" }]
          : [];
        return {
          ...base,
          state: "modified",
          sigil: isEscalation ? "↑" : "~",
          struck: false,
          isEscalation,
          showTransition: true,
          from: levelWord(status.from),
          to: levelWord(status.to),
          tags,
        };
      }
    }
  })();

  // Unresolvable membership (CONTEXT.md): only a Group's own row carries this — never the
  // Member rows beneath it, since an unresolved group derives none.
  if (isUnresolvedGroup(entry)) {
    view.tags = [...view.tags, { label: "members unresolved", kind: "unresolved" }];
  }

  // A staged edit overrides whatever the diff engine says (ADR-0022: pending state previews
  // what Apply would do, taking precedence over the read-only comparison view) — but existing
  // tags (escalation, unresolved) stay, with the pending tag prepended ahead of them.
  if (pending) {
    if (pending.kind === "level") {
      view.state = "modified";
      view.sigil = "~";
      view.struck = false;
      view.showTransition = true;
      view.from = levelWord(pending.beforeLevel);
      view.to = base.levelWord;
      view.tags = [{ label: "pending · unsaved", kind: "pending-level" }, ...view.tags];
    } else {
      view.state = "modified";
      view.sigil = "−";
      view.struck = true;
      view.showTransition = true;
      view.from = levelWord(pending.beforeLevel);
      view.to = "removed";
      view.tags = [{ label: "pending removal", kind: "pending-remove" }, ...view.tags];
    }
  }

  return view;
}
