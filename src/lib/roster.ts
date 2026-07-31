// Mirrors the JSON shape serialized by src-tauri/src/roster.rs and storage.rs. Kept in sync
// by hand — there is no shared schema between the Rust and TypeScript sides yet.

export interface Principal {
  id: string;
  label: string;
}

export type AccessType = { type: "Direct" } | { type: "Group" } | { type: "Member"; group_id: string };

// Where a grant lives (ADR-0012) — a Repo-level and a Project-level row for the same
// Principal + access type are always two distinct rows, never merged (PD-30).
export type GrantScope = "Repo" | "Project";

export type Permission = "Read" | "Write" | "CreateRepo" | "Admin";

export type RepoStatus = "Ok" | "FetchFailed";

export type DiffStatus =
  | { status: "None" }
  | { status: "Grant" }
  | { status: "Revoke" }
  | { status: "LevelChange"; from: Permission; to: Permission; kind: "Escalation" | "Demotion" };

// Repo absence/arrival (CONTEXT.md) — independent of whether the permissions fetch on either
// side succeeded (see `statusA`/`statusB`, which carry that separately).
export type RepoDiscoveryState = "present" | "absent" | "arrived";

export interface PrincipalEntry {
  principal: Principal;
  accessType: AccessType;
  scope: GrantScope;
  permission: Permission;
  diffStatus: DiffStatus;
  // Unresolvable membership (CONTEXT.md): null for Direct/Member entries — only a Group's own
  // grant carries this. `false` means the group's own grant was fetched but its member list
  // could not be, distinct from `true` (a confirmed, possibly empty, member list) — both states
  // render zero Member rows beneath the group, so this is the only thing telling them apart.
  membersResolved: boolean | null;
}

export interface RepoNode {
  repoProject: string;
  repo: string;
  statusA: RepoStatus | null;
  statusB: RepoStatus | null;
  discoveryState: RepoDiscoveryState;
  // Fetch failure (CONTEXT.md): either side's permissions call for this repo failed. When
  // true, `principals` is always empty — a fetch failure yields no reliable data to diff, so
  // the tree renders nothing rather than a pile of spurious Grants/Revokes against whichever
  // side succeeded. Distinct from a repo confirmed to have zero grants.
  fetchFailed: boolean;
  // Owning Project's own fetch (permissions-config/users and /groups) failed for this Snapshot
  // (ADR-0012) — independent of `fetchFailed`, which tracks the repo's own permissions fetch.
  // Both can be true at once.
  projectFetchFailed: boolean;
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
