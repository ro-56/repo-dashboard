// Row-level presentation logic for the roster grid (ADR-0005, ADR-0006, ADR-0007).
// Pure functions only — markup and colour live in RosterRow.svelte's scoped styles,
// keyed off the `state`/`levelClass` strings returned here, not inline style strings.

import type { AccessType, Permission, PrincipalEntry } from "./roster";

export type RowState = "same" | "added" | "removed" | "modified";
export type LevelClass = "lvl-admin" | "lvl-write" | "lvl-read";

const RANK: Record<Permission, number> = { Read: 1, Write: 2, Admin: 3 };
const SOURCE_RANK: Record<AccessType["type"], number> = { Direct: 0, Group: 1, Member: 2 };
// Exported: this glyph set is the load-bearing shape channel for permission level (ADR-0005),
// so the summary bar's per-level stats (summaryBar.ts) reuse it rather than re-literalizing it.
export const METER: Record<Permission, string> = { Admin: "■■■", Write: "■■□", Read: "■□□" };
const LEVEL_CLASS: Record<Permission, LevelClass> = { Admin: "lvl-admin", Write: "lvl-write", Read: "lvl-read" };

/** A Group's own entry whose membership could not be resolved (CONTEXT.md) — shared by the
 * row's own tag (below) and the notice strip (noticeStrip.ts) so the two can't drift apart
 * on the definition. */
export function isUnresolvedGroup(entry: PrincipalEntry): boolean {
  return entry.accessType.type === "Group" && entry.membersResolved === false;
}

/** `direct`, `group`, or `grp:<name>` — never spills into the notes column. */
export function sourceLabel(accessType: AccessType): string {
  switch (accessType.type) {
    case "Direct":
      return "direct";
    case "Group":
      return "group";
    case "Member":
      return `grp:${accessType.group_id}`;
  }
}

/** Roster ordering: admin → write → read, then principal label, then source (direct before group). */
export function compareEntries(a: PrincipalEntry, b: PrincipalEntry): number {
  return (
    RANK[b.permission] - RANK[a.permission] ||
    a.principal.label.localeCompare(b.principal.label) ||
    SOURCE_RANK[a.accessType.type] - SOURCE_RANK[b.accessType.type]
  );
}

export function sortedPrincipals(entries: PrincipalEntry[]): PrincipalEntry[] {
  return [...entries].sort(compareEntries);
}

export interface RosterRowTag {
  label: string;
  kind: "esc" | "unresolved";
}

export interface RosterRowView {
  state: RowState;
  sigil: string;
  levelWord: string;
  levelClass: LevelClass;
  meter: string;
  struck: boolean;
  isEscalation: boolean;
  showTransition: boolean;
  from: string;
  to: string;
  tags: RosterRowTag[];
}

function levelWord(permission: Permission): string {
  return permission.toLowerCase();
}

export function deriveRow(entry: PrincipalEntry): RosterRowView {
  const status = entry.diffStatus;
  const levelClass = LEVEL_CLASS[entry.permission];
  const base = {
    levelWord: levelWord(entry.permission),
    levelClass,
    meter: METER[entry.permission],
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
          ? [{ label: status.to === "Admin" ? "escalation → admin" : "escalation", kind: "esc" }]
          : [];
        return {
          ...base,
          state: "modified",
          sigil: "~",
          struck: false,
          isEscalation,
          showTransition: true,
          from: status.from.toLowerCase(),
          to: status.to.toLowerCase(),
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

  return view;
}
