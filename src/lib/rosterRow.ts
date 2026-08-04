// Row-level presentation logic for the roster grid (ADR-0005, ADR-0006, ADR-0007).
// Pure functions only — markup and colour live in RosterRow.svelte's scoped styles,
// keyed off the `state`/`levelClass` strings returned here, not inline style strings.

import type { EditableLevel, EditTarget, StagedEdit } from "./pendingEdits";
import type { AccessType, GrantScope, Permission, PrincipalEntry } from "./roster";
import type { Translate } from "./i18n/translate";

export type RowState = "same" | "added" | "removed" | "modified";
export type LevelClass = "lvl-admin" | "lvl-write" | "lvl-read";

type CoreLevel = "Read" | "Write" | "Admin";

/** Collapses `CreateRepo` onto `Admin`; every other variant passes through unchanged. Mirrors
 * `Permission::admin_light` on the Rust side (ADR-0024) — used only where code computes rank
 * or style tier, never for display (see levelWord, the display-side counterpart). */
function adminLight(permission: Permission): CoreLevel {
  return permission === "CreateRepo" ? "Admin" : permission;
}

/** The `permission.*` catalog key backing a Read/Write/Admin/CreateRepo word — shared by
 * `levelWord` (display) and `rowMenu` (menu option labels) so both name the same permission the
 * same way. */
function permissionKey(permission: Permission): string {
  return permission === "CreateRepo" ? "permission.createRepo" : `permission.${permission.toLowerCase()}`;
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
 * warning for a staged Group removal (ADR-0022): those Member rows disappear along with
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
export function cascadeNote(t: Translate, count: number | null | undefined): string | null {
  if (count === null || count === undefined) return null;
  return t("roster.menu.cascadeNote", { values: { count } });
}

/** A row carrying a diff marker (Grant/Revoke/LevelChange) — shared by the Changes only tab's
 * row-level filter and its "N changed" count (filterBar.ts) so neither can drift from the other. */
export function hasDiff(entry: PrincipalEntry): boolean {
  return entry.diffStatus.status !== "None";
}

/** Case-insensitive substring match of `query` against this single row's Principal label — a
 * blank/whitespace query always matches, mirroring filterBar.ts's repo-level `matchesSearch`
 * (a repo kept visible by a match in one row only draws that row, not the
 * whole roster, unlike an earlier "full context" behavior). */
export function matchesSearchQuery(entry: PrincipalEntry, query: string): boolean {
  const needle = query.trim().toLowerCase();
  if (!needle) return true;
  return entry.principal.label.toLowerCase().includes(needle);
}

/** Whether `entry` is hidden by the Excluded principal filter (CONTEXT.md, ADR-0032,
 * `docs/adr/0032-excluded-principal-group-cascade-scoped-to-member-rows.md`): true when its own
 * `principal.id` is in `excludedIds`, or it's a `Member` entry whose `accessType.group_id` is —
 * the one cascade, from an excluded Group's own grant to the Member rows it derives. An unrelated
 * Direct/Group entry belonging to the same person is never swept in, since it carries no reference
 * to the excluded group. */
export function isExcluded(entry: PrincipalEntry, excludedIds: ReadonlySet<string>): boolean {
  if (excludedIds.has(entry.principal.id)) return true;
  return entry.accessType.type === "Member" && excludedIds.has(entry.accessType.group_id);
}

/** `direct`, `group`, or `grp:<name>`, plus a `↳ ` prefix for Project-level grants
 * (ADR-0020) — never spills into the notes column. */
export function sourceLabel(t: Translate, accessType: AccessType, scope: GrantScope): string {
  const base = (() => {
    switch (accessType.type) {
      case "Direct":
        return t("roster.source.direct");
      case "Group":
        return t("roster.source.group");
      case "Member":
        return t("roster.source.member", { values: { group: accessType.group_id } });
    }
  })();
  return scope === "Project" ? `↳ ${base}` : base;
}

/** The `.source` cell's `title` tooltip: a one-line nudge explaining what holding the grant this
 * way means, plus a "— Project-level grant" suffix when the grant cascades from the Project
 * (ADR-0020). Deliberately not reused by `rowMenu`'s on-screen `scopeNote` (below), which keeps
 * its own bare "Project-level grant" — that note is visible menu copy, not a hover tooltip, and
 * shouldn't grow just because this one does. */
export function sourceTooltip(t: Translate, accessType: AccessType, scope: GrantScope): string {
  const base = (() => {
    switch (accessType.type) {
      case "Direct":
        return t("roster.source.directTooltip");
      case "Group":
        return t("roster.source.groupTooltip");
      case "Member":
        return t("roster.source.memberTooltip", { values: { group: accessType.group_id } });
    }
  })();
  return scope === "Project" ? t("roster.source.projectSuffix", { values: { base } }) : base;
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
  title: string;
}

function tagTitle(t: Translate, kind: RosterRowTag["kind"]): string {
  switch (kind) {
    case "esc":
      return t("roster.tag.escalationTooltip");
    case "unresolved":
      return t("roster.tag.unresolvedTooltip");
    case "pending-level":
    case "pending-remove":
      return t("roster.tag.pendingTooltip");
  }
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
  sigilTitle: string;
  levelWord: string;
  levelClass: LevelClass;
  meterTitle: string;
  struck: boolean;
  isEscalation: boolean;
  showTransition: boolean;
  from: string;
  to: string;
  tags: RosterRowTag[];
}

function sigilTitle(t: Translate, state: RowState): string {
  switch (state) {
    case "same":
      return t("roster.sigil.same");
    case "added":
      return t("roster.sigil.granted");
    case "removed":
      return t("roster.sigil.revoked");
    case "modified":
      return t("roster.sigil.changed");
  }
}

function levelClassTitle(t: Translate, cls: LevelClass): string {
  switch (cls) {
    case "lvl-admin":
      return t("permission.admin");
    case "lvl-write":
      return t("permission.write");
    case "lvl-read":
      return t("permission.read");
  }
}

const EDITABLE_LEVELS: EditableLevel[] = ["Read", "Write", "Admin"];

/** The edit target a row's menu would stage against, or `null` when this entry can't be edited
 * at all — Member rows get no menu at either scope, permanently (ADR-0021: a member's access is
 * derived from their group's own grant, and Bitbucket has no endpoint to set it independently).
 * Direct and Group entries are both editable at Repo and Project scope. */
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
  label: string;
  active: boolean;
}

export interface RowMenu {
  levelOptions: LevelMenuOption[];
  effectiveLevel: EditableLevel;
  /** The cascade note for a Project-scope grant — `null` at Repo scope. Reuses
   * whatever cascade note is "already shown elsewhere in the dashboard" rather than introducing
   * new note-computation logic ("no new note needed") — the only such note that exists today is
   * `sourceTooltip`'s "Project-level grant" string (ADR-0020's `.source`-cell tooltip), so that's
   * what's reused here rather than a newly-computed repo-count string. */
  scopeNote: string | null;
  /** How many Member grants would disappear if this Group grant were removed — `null`
   * for Direct entries and for a Group whose membership is unresolvable. See
   * `cascadeMemberCount`. */
  cascadeCount: number | null;
}

/** Builds the row menu's Read/Write/Admin options, highlighting whichever level is already
 * staged (if any) rather than the row's true current level — matching the reference design's
 * `effLevel`, so reopening the menu after staging a change shows the choice just made. Returns
 * `null` for anything `editTargetForEntry` won't produce a target for. `candidates` feeds
 * `cascadeMemberCount` for Group entries — irrelevant, and safe to omit, for Direct ones. */
export function rowMenu(
  t: Translate,
  entry: PrincipalEntry,
  staged: StagedEdit | undefined,
  candidates: PrincipalEntry[] = [],
): RowMenu | null {
  if (!editTargetForEntry(entry)) return null;
  const effectiveLevel: EditableLevel =
    staged?.request.action.type === "SetLevel" ? staged.request.action.level : (adminLight(entry.permission) as EditableLevel);
  return {
    levelOptions: EDITABLE_LEVELS.map((level) => ({
      level,
      label: t(permissionKey(level)),
      active: level === effectiveLevel,
    })),
    effectiveLevel,
    scopeNote: entry.scope === "Project" ? t("roster.menu.projectScopeNote") : null,
    cascadeCount: cascadeMemberCount(entry, candidates),
  };
}

export function levelWord(t: Translate, permission: Permission): string {
  return t(permissionKey(permission)).toLowerCase();
}

export function deriveRow(t: Translate, entry: PrincipalEntry, pending?: PendingRowView): RosterRowView {
  const status = entry.diffStatus;
  // A pending level-change previews its target level immediately (matching the reference
  // design's `effLevel`) — the meter/level-word reflect what Apply would set, not what's
  // currently live, since that's the whole point of showing it as "pending".
  const effPermission = pending?.kind === "level" && pending.afterLevel !== undefined ? pending.afterLevel : entry.permission;
  const levelClass = LEVEL_CLASS[adminLight(effPermission)];
  const base = {
    levelWord: levelWord(t, effPermission),
    levelClass,
    meterTitle: levelClassTitle(t, levelClass),
  };

  const view: RosterRowView = (() => {
    switch (status.status) {
      case "None":
        return {
          ...base,
          state: "same",
          sigil: "·",
          sigilTitle: sigilTitle(t, "same"),
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
          sigilTitle: sigilTitle(t, "added"),
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
          sigilTitle: sigilTitle(t, "removed"),
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
          ? [
              {
                label: adminLight(status.to) === "Admin" ? t("roster.tag.escalationToAdmin") : t("roster.tag.escalation"),
                kind: "esc",
                title: tagTitle(t, "esc"),
              },
            ]
          : [];
        return {
          ...base,
          state: "modified",
          sigil: isEscalation ? "↑" : "~",
          sigilTitle: isEscalation ? t("roster.sigil.escalated") : sigilTitle(t, "modified"),
          struck: false,
          isEscalation,
          showTransition: true,
          from: levelWord(t, status.from),
          to: levelWord(t, status.to),
          tags,
        };
      }
    }
  })();

  // Unresolvable membership (CONTEXT.md): only a Group's own row carries this — never the
  // Member rows beneath it, since an unresolved group derives none.
  if (isUnresolvedGroup(entry)) {
    view.tags = [...view.tags, { label: t("roster.tag.unresolved"), kind: "unresolved", title: tagTitle(t, "unresolved") }];
  }

  // A staged edit overrides whatever the diff engine says (ADR-0022: pending state previews
  // what Apply would do, taking precedence over the read-only comparison view) — but existing
  // tags (escalation, unresolved) stay, with the pending tag prepended ahead of them.
  if (pending) {
    if (pending.kind === "level") {
      view.state = "modified";
      view.sigil = "~";
      // "Changed"/"Revoked" (sigilTitle) name diff outcomes between two Snapshots — a Pending
      // edit is neither (CONTEXT.md: Remove grant is deliberately distinct from Revoke), so it
      // gets its own wording rather than borrowing one that implies this already happened.
      view.sigilTitle = t("roster.sigil.pendingChange");
      view.struck = false;
      view.showTransition = true;
      view.from = levelWord(t, pending.beforeLevel);
      view.to = base.levelWord;
      view.tags = [
        { label: t("roster.tag.pendingLevel"), kind: "pending-level", title: tagTitle(t, "pending-level") },
        ...view.tags,
      ];
    } else {
      view.state = "modified";
      view.sigil = "−";
      view.sigilTitle = t("roster.sigil.pendingRemoval");
      view.struck = true;
      view.showTransition = true;
      view.from = levelWord(t, pending.beforeLevel);
      view.to = t("roster.pendingRemovedTo");
      view.tags = [
        { label: t("roster.tag.pendingRemove"), kind: "pending-remove", title: tagTitle(t, "pending-remove") },
        ...view.tags,
      ];
    }
  }

  return view;
}
