// Mirrors the JSON shape serialized by src-tauri/src/roster.rs and storage.rs. Kept in sync
// by hand — there is no shared schema between the Rust and TypeScript sides yet.

export interface Principal {
  id: string;
  label: string;
}

export type AccessType = { type: "Direct" } | { type: "Group" } | { type: "Member"; group_id: string };

export type Permission = "Read" | "Write" | "Admin";

export type RepoStatus = "Ok" | "FetchFailed";

export type DiffStatus =
  | { status: "None" }
  | { status: "Grant" }
  | { status: "Revoke" }
  | { status: "LevelChange"; from: Permission; to: Permission; kind: "Escalation" | "Demotion" };

export interface PrincipalEntry {
  principal: Principal;
  accessType: AccessType;
  permission: Permission;
  diffStatus: DiffStatus;
}

export interface RepoNode {
  repoProject: string;
  repo: string;
  statusA: RepoStatus | null;
  statusB: RepoStatus | null;
  principals: PrincipalEntry[];
  readCount: number;
  writeCount: number;
  adminCount: number;
  changeCount: number;
}

export interface ProjectNode {
  repoProject: string;
  repos: RepoNode[];
}

export type RosterTree = ProjectNode[];

export interface SnapshotSummary {
  id: number;
  runAt: string;
}
