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

// Workspace departure/arrival (CONTEXT.md) is a "user Principal" concept only — `null` for a
// `Group`'s own grant, since a group is never itself a user.
export type WorkspaceState = "present" | "departed" | "arrived";

// Repo absence/arrival (CONTEXT.md) — independent of whether the permissions fetch on either
// side succeeded (see `statusA`/`statusB`, which carry that separately).
export type RepoDiscoveryState = "present" | "absent" | "arrived";

export interface PrincipalEntry {
  principal: Principal;
  accessType: AccessType;
  permission: Permission;
  diffStatus: DiffStatus;
  workspaceState: WorkspaceState | null;
}

export interface RepoNode {
  repoProject: string;
  repo: string;
  statusA: RepoStatus | null;
  statusB: RepoStatus | null;
  discoveryState: RepoDiscoveryState;
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

export interface LevelCounts {
  admin: number;
  write: number;
  read: number;
}

// Aggregate facts about the Comparison Snapshot alone — always the full Snapshot, never
// affected by any view/filter state (ADR-0008: every count is over PermissionRecord rows).
export interface ComparisonSummary {
  totalGrants: number;
  distinctRepos: number;
  distinctUsers: number;
  levels: LevelCounts;
}

// Aggregate facts about the whole Run pair (Baseline -> Comparison) — always the full pair,
// never affected by any view/filter state.
export interface PairStats {
  added: number;
  revoked: number;
  changed: number;
  escalations: number;
  reposHit: number;
  net: number;
}

export interface RosterTreeResult {
  tree: RosterTree;
  comparison: ComparisonSummary;
  pair: PairStats;
}
