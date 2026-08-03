// Diff-free CSV export of a single Snapshot's full roster (ADR-0026). Pure
// functions only — plugin wiring (Save As dialog, file write) lives in HeadBar.svelte/the
// dashboard page.

import { levelWord } from "./rosterRow";
import type { AccessType, RosterTree } from "./roster";
import { englishTranslate } from "./i18n/englishTranslate";

const CSV_HEADER = ["repo_project", "repo", "username", "display_name", "access_type", "scope", "permission", "run_at"];

/** RFC 4180-style quoting: only fields containing a comma, quote, or newline get wrapped, with
 * embedded quotes doubled. Everything else passes through unquoted. */
export function escapeCsvField(field: string): string {
  if (/[",\n\r]/.test(field)) {
    return `"${field.replace(/"/g, '""')}"`;
  }
  return field;
}

/** `direct`, `group`, or `member:<group_id>` (mirrors `AccessType`'s wire shape). */
function accessTypeCsv(accessType: AccessType): string {
  switch (accessType.type) {
    case "Direct":
      return "direct";
    case "Group":
      return "group";
    case "Member":
      return `member:${accessType.group_id}`;
  }
}

/** Flattens every `PrincipalEntry` in `tree` into one CSV row each. Ignores `diffStatus` and
 * view-mode filtering entirely — every row is a plain current-state row for `runAt`,
 * regardless of what diff markers or filters the input tree happens to carry. */
export function rosterToCsv(tree: RosterTree, runAt: string): string {
  const rows: string[][] = [CSV_HEADER];

  for (const project of tree) {
    for (const repo of project.repos) {
      for (const entry of repo.principals) {
        rows.push([
          repo.repoProject,
          repo.repo,
          entry.principal.id,
          entry.principal.label,
          accessTypeCsv(entry.accessType),
          entry.scope === "Repo" ? "repo" : "project",
          levelWord(englishTranslate, entry.permission),
          runAt,
        ]);
      }
    }
  }

  return rows.map((row) => row.map(escapeCsvField).join(",")).join("\n");
}

/** `roster_<workspace>_<timestamp>.csv`, with `runAt`'s colons swapped for dashes so the
 * result is a valid filename on every OS. */
export function exportFilename(workspace: string, runAt: string): string {
  return `roster_${workspace}_${runAt.replace(/:/g, "-")}.csv`;
}
