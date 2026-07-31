//! The Roster tree (ADR-0004): for a chosen Run pair, loads both Snapshots, runs the
//! existing diff engine (`diff.rs`, unmodified) for Grant/Revoke/Level-change entries, then
//! folds in the unchanged records so every Principal — changed or not — appears in the
//! tree. Computed fresh on every call; nothing here is persisted or cached.

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;
use serde::Serialize;

use crate::diff::{diff_snapshots, DiffResult, LevelChangeKind, RecordDiff, Side, Snapshot};
use crate::model::{
    AccessType, GrantScope, GroupMembershipStatus, Permission, PermissionRecord, ProjectFetchStatus,
    RepoFetchStatus, RepoStatus,
};
use crate::storage::{
    load_group_membership_statuses, load_project_fetch_statuses, load_snapshot,
};

/// Mirrors `diff.rs`'s private record key exactly (ADR-0012: `scope` is part of it), so a
/// record counted as "changed" by the diff engine is never also folded in again as unchanged.
type RecordKey = (String, String, GrantScope, AccessType);

type RepoKey = (String, String);

fn record_key(r: &PermissionRecord) -> RecordKey {
    (r.repo.clone(), r.principal.id.clone(), r.scope, r.access_type.clone())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status")]
pub enum DiffStatus {
    None,
    Grant,
    Revoke,
    LevelChange {
        from: Permission,
        to: Permission,
        kind: LevelChangeKind,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrincipalEntry {
    pub principal: crate::model::Principal,
    pub access_type: AccessType,
    /// Where this grant lives (ADR-0012) — `Repo` or `Project`. Part of the diff key
    /// (`diff.rs`), so a Repo-level and a Project-level row for the same Principal + access
    /// type are always two distinct rows here, never merged.
    pub scope: GrantScope,
    /// The permission currently on display: Snapshot B's value for Grant/None/LevelChange,
    /// Snapshot A's value for Revoke (there is no B-side value for a revoked record).
    pub permission: Permission,
    pub diff_status: DiffStatus,
    /// Unresolvable membership (CONTEXT.md): `None` for `Direct`/`Member` entries — only a
    /// `Group`'s own grant carries this. `Some(false)` means the group's own grant was fetched
    /// but its member list could not be, distinct from `Some(true)` (a confirmed, possibly
    /// empty, member list) — both states have zero derived `Member` rows in this tree, so this
    /// field is the only thing that tells them apart. Sourced from whichever Snapshot side this
    /// entry's `permission` was read from (B for Grant/None/LevelChange, A for Revoke), mirroring
    /// that field's own side selection.
    pub members_resolved: Option<bool>,
}

/// Repo arrival / absence (CONTEXT.md): whether this repo's discovery — independent of
/// whether its permissions fetch succeeded — spans the whole pair or only one side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RepoDiscoveryState {
    Present,
    /// Discovered in the Baseline, not in the Comparison — every grant on it reads as a Revoke.
    Absent,
    /// Discovered in the Comparison, not in the Baseline — every grant on it reads as a Grant.
    Arrived,
}

/// Per-repo, per-Snapshot fetch/discovery state, `None` meaning absent from that Snapshot's
/// discovery entirely (see `model::RepoFetchStatus`) — attached even when unchanged between
/// A and B, since the collapsed card needs it regardless of whether it's a diffed value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoNode {
    pub repo_project: String,
    pub repo: String,
    pub status_a: Option<RepoStatus>,
    pub status_b: Option<RepoStatus>,
    pub discovery_state: RepoDiscoveryState,
    /// Fetch failure (CONTEXT.md): true when either side's permissions call for this repo
    /// failed. `principals` is forced empty whenever this is true — a fetch failure yields no
    /// reliable data to diff, so the raw diff engine's output for this repo (which would
    /// otherwise read as a pile of spurious Grants/Revokes against the side that succeeded) is
    /// discarded in favour of rendering nothing, distinct from a repo confirmed to have zero
    /// grants.
    pub fetch_failed: bool,
    /// True when this repo's owning Project's own fetch (`ProjectFetchStatus::FetchFailed`,
    /// ADR-0012) failed on either Snapshot side, joined at query time by `repo_project` (the
    /// Project key). Independent of `status_a`/`status_b`/`fetch_failed` above, which describe
    /// this repo's *own* fetch — a repo can be `Ok` at the repo level while its Project is
    /// unknown, so unlike `fetch_failed` this never clears `principals`.
    pub project_fetch_failed: bool,
    pub principals: Vec<PrincipalEntry>,
    /// Counts reflect currently-held access (i.e. exclude Revoked entries) — a Grant, an
    /// unchanged record, and the "to" side of a Level change all count; a Revoke does not,
    /// since that Principal no longer holds that permission as of Snapshot B.
    pub read_count: usize,
    pub write_count: usize,
    pub admin_count: usize,
    /// Count of Grant + Revoke + Level-change entries in this repo. Zero whenever A and B
    /// are the same Snapshot, and forced to zero when `fetch_failed` is true.
    pub change_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectNode {
    pub repo_project: String,
    pub repos: Vec<RepoNode>,
}

pub type RosterTree = Vec<ProjectNode>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelCounts {
    pub admin: usize,
    pub write: usize,
    pub read: usize,
}

/// Aggregate facts about the Comparison Snapshot alone. Always computed over the full
/// Snapshot, never affected by any view/filter state; every count is over `PermissionRecord`
/// rows, not people (ADR-0008).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComparisonSummary {
    pub total_grants: usize,
    /// Distinct repos discovered in the Comparison Snapshot (Ok or FetchFailed alike) —
    /// sourced from `repo_statuses`, not `records`, so a FetchFailed or genuinely-empty repo
    /// still counts.
    pub distinct_repos: usize,
    /// Distinct Direct/Member principal ids — a `Group`'s own grant does not make it a "user
    /// Principal" (CONTEXT.md).
    pub distinct_users: usize,
    pub levels: LevelCounts,
}

/// Aggregate facts about the whole Run pair (Baseline -> Comparison). Always computed over
/// the full pair, never affected by any view/filter state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairStats {
    pub added: usize,
    pub revoked: usize,
    pub changed: usize,
    pub escalations: usize,
    /// Repos with at least one Grant, Revoke, or Level-change entry.
    pub repos_hit: usize,
    /// Comparison grant count minus Baseline grant count, signed.
    pub net: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RosterTreeResult {
    pub tree: RosterTree,
    pub comparison: ComparisonSummary,
    pub pair: PairStats,
}

fn repo_status_map(statuses: &[RepoFetchStatus]) -> HashMap<RepoKey, RepoStatus> {
    statuses
        .iter()
        .map(|s| ((s.repo_project.clone(), s.repo.clone()), s.status))
        .collect()
}

/// Keyed on `project_key` (== `repo_project`, ADR-0012) — a Project's own fetch outcome for
/// one Snapshot side, joined onto every repo it owns at query time.
fn project_status_map(statuses: &[ProjectFetchStatus]) -> HashMap<String, RepoStatus> {
    statuses.iter().map(|s| (s.project_key.clone(), s.status)).collect()
}

type GroupKey = (String, String, String);

/// Keyed on `(repo_project, repo, group_id)` — a `Group` grant's own membership-resolution
/// fact for one Snapshot side. `normalize_repo_group_permissions` always writes exactly one of
/// these alongside a `Group` `PermissionRecord`, so a lookup miss should not happen in
/// practice; `resolved_or_default` treats it as resolved rather than surfacing a phantom tag.
fn membership_status_map(statuses: &[GroupMembershipStatus]) -> HashMap<GroupKey, bool> {
    statuses
        .iter()
        .map(|s| ((s.repo_project.clone(), s.repo.clone(), s.group_id.clone()), s.members_resolved))
        .collect()
}

/// A `Group`'s own grant is never a "user Principal" (CONTEXT.md) — this is the one place
/// that distinction is drawn, shared by every distinct-user count in this module.
fn is_user_access_type(access_type: &AccessType) -> bool {
    !matches!(access_type, AccessType::Group)
}

/// Distinct Direct/Member principal ids appearing anywhere in `records` — the "user
/// Principal" population a `Group`'s own grant is deliberately excluded from.
fn user_principal_ids(records: &[PermissionRecord]) -> HashSet<String> {
    records
        .iter()
        .filter(|r| is_user_access_type(&r.access_type))
        .map(|r| r.principal.id.clone())
        .collect()
}

fn comparison_summary(b: &Snapshot) -> ComparisonSummary {
    let total_grants = b.records.len();

    let distinct_repos = b
        .repo_statuses
        .iter()
        .map(|s| (s.repo_project.clone(), s.repo.clone()))
        .collect::<HashSet<RepoKey>>()
        .len();

    let distinct_users = user_principal_ids(&b.records).len();

    let mut levels = LevelCounts::default();
    for r in &b.records {
        match r.permission {
            Permission::Read => levels.read += 1,
            Permission::Write => levels.write += 1,
            Permission::Admin => levels.admin += 1,
        }
    }

    ComparisonSummary { total_grants, distinct_repos, distinct_users, levels }
}

fn pair_stats(diff: &DiffResult, a: &Snapshot, b: &Snapshot) -> PairStats {
    let mut added = 0usize;
    let mut revoked = 0usize;
    let mut changed = 0usize;
    let mut escalations = 0usize;
    let mut repos_hit: HashSet<(String, String)> = HashSet::new();

    for record_diff in &diff.records {
        match record_diff {
            RecordDiff::Grant(r) => {
                added += 1;
                repos_hit.insert((r.repo_project.clone(), r.repo.clone()));
            }
            RecordDiff::Revoke(r) => {
                revoked += 1;
                repos_hit.insert((r.repo_project.clone(), r.repo.clone()));
            }
            RecordDiff::LevelChange { repo_project, repo, kind, .. } => {
                changed += 1;
                if *kind == LevelChangeKind::Escalation {
                    escalations += 1;
                }
                repos_hit.insert((repo_project.clone(), repo.clone()));
            }
        }
    }

    let net = b.records.len() as i64 - a.records.len() as i64;

    PairStats { added, revoked, changed, escalations, repos_hit: repos_hit.len(), net }
}

fn build_roster_tree(
    a: &Snapshot,
    b: &Snapshot,
    diff: &DiffResult,
    membership_a: &[GroupMembershipStatus],
    membership_b: &[GroupMembershipStatus],
    project_statuses_a: &[ProjectFetchStatus],
    project_statuses_b: &[ProjectFetchStatus],
) -> RosterTree {
    let membership_map_a = membership_status_map(membership_a);
    let membership_map_b = membership_status_map(membership_b);
    // A `Group`'s own grant has no group id in `AccessType::Group` itself — for that access
    // type, `principal.id` *is* the group's slug (model.rs). Mirrors `permission`'s own side
    // selection (B for Grant/None/LevelChange, A for Revoke) — the membership fact travels with
    // whichever side's value is on display.
    let members_resolved =
        |access_type: &AccessType, principal_id: &str, repo_project: &str, repo: &str, side: Side| -> Option<bool> {
            match access_type {
                AccessType::Group => {
                    let map = match side {
                        Side::B => &membership_map_b,
                        Side::A => &membership_map_a,
                    };
                    let key = (repo_project.to_string(), repo.to_string(), principal_id.to_string());
                    Some(map.get(&key).copied().unwrap_or(true))
                }
                AccessType::Direct | AccessType::Member(_) => None,
            }
        };

    let mut by_repo: HashMap<(String, String), Vec<PrincipalEntry>> = HashMap::new();
    let mut changed_keys: HashSet<RecordKey> = HashSet::new();

    for record_diff in &diff.records {
        let (repo_project, repo, entry) = match record_diff {
            RecordDiff::Grant(r) => {
                changed_keys.insert(record_key(r));
                (
                    r.repo_project.clone(),
                    r.repo.clone(),
                    PrincipalEntry {
                        principal: r.principal.clone(),
                        access_type: r.access_type.clone(),
                        scope: r.scope,
                        permission: r.permission,
                        diff_status: DiffStatus::Grant,
                        members_resolved: members_resolved(
                            &r.access_type,
                            &r.principal.id,
                            &r.repo_project,
                            &r.repo,
                            Side::B,
                        ),
                    },
                )
            }
            RecordDiff::Revoke(r) => {
                changed_keys.insert(record_key(r));
                (
                    r.repo_project.clone(),
                    r.repo.clone(),
                    PrincipalEntry {
                        principal: r.principal.clone(),
                        access_type: r.access_type.clone(),
                        scope: r.scope,
                        permission: r.permission,
                        diff_status: DiffStatus::Revoke,
                        members_resolved: members_resolved(
                            &r.access_type,
                            &r.principal.id,
                            &r.repo_project,
                            &r.repo,
                            Side::A,
                        ),
                    },
                )
            }
            RecordDiff::LevelChange { repo_project, repo, principal, access_type, scope, from, to, kind } => {
                changed_keys.insert((repo.clone(), principal.id.clone(), *scope, access_type.clone()));
                (
                    repo_project.clone(),
                    repo.clone(),
                    PrincipalEntry {
                        principal: principal.clone(),
                        access_type: access_type.clone(),
                        scope: *scope,
                        permission: *to,
                        diff_status: DiffStatus::LevelChange { from: *from, to: *to, kind: *kind },
                        members_resolved: members_resolved(access_type, &principal.id, repo_project, repo, Side::B),
                    },
                )
            }
        };
        by_repo.entry((repo_project, repo)).or_default().push(entry);
    }

    // Any key present in b.records that the diff engine didn't flag as changed is, by
    // construction, present in both Snapshots with the same permission.
    for r in &b.records {
        if changed_keys.contains(&record_key(r)) {
            continue;
        }
        by_repo.entry((r.repo_project.clone(), r.repo.clone())).or_default().push(PrincipalEntry {
            principal: r.principal.clone(),
            access_type: r.access_type.clone(),
            scope: r.scope,
            permission: r.permission,
            diff_status: DiffStatus::None,
            members_resolved: members_resolved(&r.access_type, &r.principal.id, &r.repo_project, &r.repo, Side::B),
        });
    }

    let status_a = repo_status_map(&a.repo_statuses);
    let status_b = repo_status_map(&b.repo_statuses);
    let project_status_a = project_status_map(project_statuses_a);
    let project_status_b = project_status_map(project_statuses_b);

    let mut repo_keys: HashSet<(String, String)> = HashSet::new();
    repo_keys.extend(status_a.keys().cloned());
    repo_keys.extend(status_b.keys().cloned());
    repo_keys.extend(by_repo.keys().cloned());

    let mut repos_by_project: HashMap<String, Vec<RepoNode>> = HashMap::new();

    for key in repo_keys {
        let (repo_project, repo) = key.clone();

        // Fetch failure (CONTEXT.md): either side's permissions call for this repo errored.
        // The diff engine has no way to know that — a repo Ok on one side and FetchFailed on
        // the other produces a pile of spurious Grant/Revoke entries against the side that
        // succeeded, which would misrepresent an API failure as real access changes. Discard
        // whatever `by_repo` computed for this key and render nothing.
        let fetch_failed = matches!(status_a.get(&key), Some(RepoStatus::FetchFailed))
            || matches!(status_b.get(&key), Some(RepoStatus::FetchFailed));

        // Project fetch failure (ADR-0012): joined by `repo_project` (the Project key), on
        // either Snapshot side. Unlike `fetch_failed` above, this never discards `principals` —
        // this repo's own data, if it fetched successfully, stays fully visible; only the
        // Project layer above it is flagged unknown.
        let project_fetch_failed = matches!(project_status_a.get(&repo_project), Some(RepoStatus::FetchFailed))
            || matches!(project_status_b.get(&repo_project), Some(RepoStatus::FetchFailed));

        let mut principals = if fetch_failed { Vec::new() } else { by_repo.remove(&key).unwrap_or_default() };
        principals.sort_by(|x, y| {
            x.principal.label.cmp(&y.principal.label).then(x.principal.id.cmp(&y.principal.id))
        });

        let mut read_count = 0usize;
        let mut write_count = 0usize;
        let mut admin_count = 0usize;
        let mut change_count = 0usize;
        for p in &principals {
            if p.diff_status != DiffStatus::None {
                change_count += 1;
            }
            if p.diff_status != DiffStatus::Revoke {
                match p.permission {
                    Permission::Read => read_count += 1,
                    Permission::Write => write_count += 1,
                    Permission::Admin => admin_count += 1,
                }
            }
        }

        let discovery_state = match (status_a.get(&key), status_b.get(&key)) {
            (Some(_), Some(_)) => RepoDiscoveryState::Present,
            (Some(_), None) => RepoDiscoveryState::Absent,
            (None, Some(_)) => RepoDiscoveryState::Arrived,
            (None, None) => RepoDiscoveryState::Present,
        };

        repos_by_project.entry(repo_project.clone()).or_default().push(RepoNode {
            repo_project,
            repo: repo.clone(),
            status_a: status_a.get(&key).copied(),
            status_b: status_b.get(&key).copied(),
            discovery_state,
            fetch_failed,
            project_fetch_failed,
            principals,
            read_count,
            write_count,
            admin_count,
            change_count,
        });
    }

    let mut projects: RosterTree = repos_by_project
        .into_iter()
        .map(|(repo_project, mut repos)| {
            repos.sort_by(|x, y| x.repo.cmp(&y.repo));
            ProjectNode { repo_project, repos }
        })
        .collect();
    projects.sort_by(|x, y| x.repo_project.cmp(&y.repo_project));
    projects
}

/// The test seam: loads Snapshot A and B via `storage::load_snapshot`, runs the diff engine
/// unmodified, and returns the fully grouped, diff-annotated Roster tree alongside the
/// workspace-summary and pair-stat aggregates (PD-17). Selecting the same Snapshot for both
/// ids is a valid Run pair — every entry comes back with `DiffStatus::None` and every
/// pair-level stat is zero.
pub fn get_roster_tree(
    conn: &Connection,
    snapshot_a_id: i64,
    snapshot_b_id: i64,
) -> rusqlite::Result<RosterTreeResult> {
    let a = load_snapshot(conn, snapshot_a_id)?;
    let b = load_snapshot(conn, snapshot_b_id)?;
    let diff = diff_snapshots(&a, &b);

    let membership_a = load_group_membership_statuses(conn, snapshot_a_id)?;
    let membership_b = load_group_membership_statuses(conn, snapshot_b_id)?;
    let project_statuses_a = load_project_fetch_statuses(conn, snapshot_a_id)?;
    let project_statuses_b = load_project_fetch_statuses(conn, snapshot_b_id)?;

    Ok(RosterTreeResult {
        tree: build_roster_tree(
            &a,
            &b,
            &diff,
            &membership_a,
            &membership_b,
            &project_statuses_a,
            &project_statuses_b,
        ),
        comparison: comparison_summary(&b),
        pair: pair_stats(&diff, &a, &b),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{GrantScope, Principal};
    use crate::storage::{init_schema, save_snapshot};

    fn open_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        conn
    }

    fn record(
        repo_project: &str,
        repo: &str,
        account_id: &str,
        label: &str,
        permission: Permission,
    ) -> PermissionRecord {
        PermissionRecord {
            repo_project: repo_project.to_string(),
            repo: repo.to_string(),
            principal: Principal { id: account_id.to_string(), label: label.to_string() },
            access_type: AccessType::Direct,
            scope: GrantScope::Repo,
            permission,
        }
    }

    fn member_record(
        repo_project: &str,
        repo: &str,
        group_id: &str,
        account_id: &str,
        label: &str,
        permission: Permission,
    ) -> PermissionRecord {
        PermissionRecord {
            repo_project: repo_project.to_string(),
            repo: repo.to_string(),
            principal: Principal { id: account_id.to_string(), label: label.to_string() },
            access_type: AccessType::Member(group_id.to_string()),
            scope: GrantScope::Repo,
            permission,
        }
    }

    fn group_record(
        repo_project: &str,
        repo: &str,
        group_id: &str,
        label: &str,
        permission: Permission,
    ) -> PermissionRecord {
        PermissionRecord {
            repo_project: repo_project.to_string(),
            repo: repo.to_string(),
            principal: Principal { id: group_id.to_string(), label: label.to_string() },
            access_type: AccessType::Group,
            scope: GrantScope::Repo,
            permission,
        }
    }

    /// Mirrors `record`, but scoped to the owning Project (ADR-0012) rather than the repo itself.
    fn project_record(
        repo_project: &str,
        repo: &str,
        account_id: &str,
        label: &str,
        permission: Permission,
    ) -> PermissionRecord {
        PermissionRecord { scope: GrantScope::Project, ..record(repo_project, repo, account_id, label, permission) }
    }

    fn project_ok_status(project_key: &str) -> ProjectFetchStatus {
        ProjectFetchStatus { project_key: project_key.to_string(), status: RepoStatus::Ok }
    }

    fn project_fetch_failed_status(project_key: &str) -> ProjectFetchStatus {
        ProjectFetchStatus { project_key: project_key.to_string(), status: RepoStatus::FetchFailed }
    }

    fn ok_status(repo_project: &str, repo: &str) -> RepoFetchStatus {
        RepoFetchStatus {
            repo_project: repo_project.to_string(),
            repo: repo.to_string(),
            status: RepoStatus::Ok,
        }
    }

    fn fetch_failed_status(repo_project: &str, repo: &str) -> RepoFetchStatus {
        RepoFetchStatus {
            repo_project: repo_project.to_string(),
            repo: repo.to_string(),
            status: RepoStatus::FetchFailed,
        }
    }

    fn membership_status(
        repo_project: &str,
        repo: &str,
        group_id: &str,
        members_resolved: bool,
    ) -> GroupMembershipStatus {
        GroupMembershipStatus {
            repo_project: repo_project.to_string(),
            repo: repo.to_string(),
            group_id: group_id.to_string(),
            members_resolved,
        }
    }

    fn repo_node<'a>(tree: &'a RosterTree, repo_project: &str, repo: &str) -> &'a RepoNode {
        tree.iter()
            .find(|p| p.repo_project == repo_project)
            .and_then(|p| p.repos.iter().find(|r| r.repo == repo))
            .expect("repo node present in tree")
    }

    fn principal_entry<'a>(repo: &'a RepoNode, label: &str) -> &'a PrincipalEntry {
        repo.principals
            .iter()
            .find(|p| p.principal.label == label)
            .expect("principal present in repo")
    }

    fn principal_entry_with_scope<'a>(repo: &'a RepoNode, label: &str, scope: GrantScope) -> &'a PrincipalEntry {
        repo.principals
            .iter()
            .find(|p| p.principal.label == label && p.scope == scope)
            .expect("principal with given scope present in repo")
    }

    #[test]
    fn mix_of_grants_revokes_and_level_changes_produces_correct_tree() {
        let mut conn = open_conn();

        let a = Snapshot {
            records: vec![
                record("TEAM", "repo-a", "acct-1", "Ada", Permission::Read), // revoked in b
                record("TEAM", "repo-a", "acct-2", "Grace", Permission::Write), // escalated in b
                record("TEAM", "repo-a", "acct-3", "Linus", Permission::Read), // unchanged
            ],
            repo_statuses: vec![ok_status("TEAM", "repo-a")],
        };
        let b = Snapshot {
            records: vec![
                record("TEAM", "repo-a", "acct-2", "Grace", Permission::Admin),
                record("TEAM", "repo-a", "acct-3", "Linus", Permission::Read),
                record("TEAM", "repo-a", "acct-4", "Margaret", Permission::Write), // granted in b
            ],
            repo_statuses: vec![ok_status("TEAM", "repo-a")],
        };

        let a_id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &a, &[], &[]).unwrap();
        let b_id = save_snapshot(&mut conn, "2026-01-02T00:00:00Z", &b, &[], &[]).unwrap();

        let result = get_roster_tree(&conn, a_id, b_id).unwrap();
        let tree = result.tree;
        assert_eq!(tree.len(), 1);
        let repo = repo_node(&tree, "TEAM", "repo-a");

        assert_eq!(repo.change_count, 3); // revoke + escalation + grant
        // currently-held: Grace(admin) + Linus(read) + Margaret(write) — Ada revoked, excluded
        assert_eq!(repo.read_count, 1);
        assert_eq!(repo.write_count, 1);
        assert_eq!(repo.admin_count, 1);

        let statuses: HashMap<&str, &DiffStatus> =
            repo.principals.iter().map(|p| (p.principal.label.as_str(), &p.diff_status)).collect();
        assert_eq!(statuses["Ada"], &DiffStatus::Revoke);
        assert_eq!(
            statuses["Grace"],
            &DiffStatus::LevelChange {
                from: Permission::Write,
                to: Permission::Admin,
                kind: LevelChangeKind::Escalation,
            }
        );
        assert_eq!(statuses["Linus"], &DiffStatus::None);
        assert_eq!(statuses["Margaret"], &DiffStatus::Grant);
    }

    #[test]
    fn identical_snapshot_run_pair_produces_zero_diff_markers() {
        let mut conn = open_conn();
        let snapshot = Snapshot {
            records: vec![
                record("TEAM", "repo-a", "acct-1", "Ada", Permission::Read),
                record("TEAM", "repo-a", "acct-2", "Grace", Permission::Admin),
            ],
            repo_statuses: vec![ok_status("TEAM", "repo-a")],
        };
        let id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &snapshot, &[], &[]).unwrap();

        let result = get_roster_tree(&conn, id, id).unwrap();
        let repo = repo_node(&result.tree, "TEAM", "repo-a");

        assert_eq!(repo.change_count, 0);
        assert!(repo.principals.iter().all(|p| p.diff_status == DiffStatus::None));
        assert_eq!(repo.read_count, 1);
        assert_eq!(repo.admin_count, 1);
    }

    #[test]
    fn repo_absent_from_one_snapshots_discovery_is_flagged_at_the_repo_level() {
        let mut conn = open_conn();
        let a = Snapshot {
            records: vec![],
            repo_statuses: vec![ok_status("TEAM", "repo-a"), ok_status("TEAM", "repo-b")],
        };
        let b = Snapshot {
            records: vec![],
            // repo-b never discovered this run
            repo_statuses: vec![ok_status("TEAM", "repo-a")],
        };

        let a_id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &a, &[], &[]).unwrap();
        let b_id = save_snapshot(&mut conn, "2026-01-02T00:00:00Z", &b, &[], &[]).unwrap();

        let result = get_roster_tree(&conn, a_id, b_id).unwrap();
        let repo_a = repo_node(&result.tree, "TEAM", "repo-a");
        let repo_b = repo_node(&result.tree, "TEAM", "repo-b");

        assert_eq!(repo_b.status_a, Some(RepoStatus::Ok));
        assert_eq!(repo_b.status_b, None);
        assert_eq!(repo_a.discovery_state, RepoDiscoveryState::Present);
        assert_eq!(repo_b.discovery_state, RepoDiscoveryState::Absent);
    }

    #[test]
    fn repo_only_in_comparison_is_flagged_arrived() {
        let mut conn = open_conn();
        let a = Snapshot { records: vec![], repo_statuses: vec![ok_status("TEAM", "repo-a")] };
        let b = Snapshot {
            records: vec![],
            // repo-c discovered for the first time in the Comparison
            repo_statuses: vec![ok_status("TEAM", "repo-a"), ok_status("TEAM", "repo-c")],
        };

        let a_id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &a, &[], &[]).unwrap();
        let b_id = save_snapshot(&mut conn, "2026-01-02T00:00:00Z", &b, &[], &[]).unwrap();

        let result = get_roster_tree(&conn, a_id, b_id).unwrap();
        let repo_c = repo_node(&result.tree, "TEAM", "repo-c");

        assert_eq!(repo_c.status_a, None);
        assert_eq!(repo_c.status_b, Some(RepoStatus::Ok));
        assert_eq!(repo_c.discovery_state, RepoDiscoveryState::Arrived);
    }

    #[test]
    fn comparison_summary_counts_grant_rows_not_people_across_multiple_sources() {
        // ADR-0008: a user holding a Direct read and a Member admin on one repo contributes
        // two grants and one distinct user, not two.
        let mut conn = open_conn();
        let b = Snapshot {
            records: vec![
                record("TEAM", "repo-a", "acct-1", "Ada", Permission::Read),
                member_record("TEAM", "repo-a", "platform-eng", "acct-1", "Ada", Permission::Admin),
                group_record("TEAM", "repo-a", "platform-eng", "Platform Engineering", Permission::Write),
            ],
            repo_statuses: vec![ok_status("TEAM", "repo-a")],
        };
        let a_id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &Snapshot::default(), &[], &[]).unwrap();
        let b_id = save_snapshot(&mut conn, "2026-01-02T00:00:00Z", &b, &[], &[]).unwrap();

        let result = get_roster_tree(&conn, a_id, b_id).unwrap();

        assert_eq!(result.comparison.total_grants, 3);
        assert_eq!(result.comparison.distinct_repos, 1);
        // acct-1 counted once despite two grant sources; the group's own grant isn't a "user".
        assert_eq!(result.comparison.distinct_users, 1);
        assert_eq!(result.comparison.levels.read, 1);
        assert_eq!(result.comparison.levels.write, 1);
        assert_eq!(result.comparison.levels.admin, 1);
    }

    #[test]
    fn net_goes_negative_when_comparison_has_fewer_grants_than_baseline() {
        let mut conn = open_conn();
        let a = Snapshot {
            records: vec![
                record("TEAM", "repo-a", "acct-1", "Ada", Permission::Read),
                record("TEAM", "repo-a", "acct-2", "Grace", Permission::Write),
                record("TEAM", "repo-a", "acct-3", "Linus", Permission::Admin),
            ],
            repo_statuses: vec![ok_status("TEAM", "repo-a")],
        };
        let b = Snapshot {
            records: vec![record("TEAM", "repo-a", "acct-1", "Ada", Permission::Read)],
            repo_statuses: vec![ok_status("TEAM", "repo-a")],
        };

        let a_id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &a, &[], &[]).unwrap();
        let b_id = save_snapshot(&mut conn, "2026-01-02T00:00:00Z", &b, &[], &[]).unwrap();

        let result = get_roster_tree(&conn, a_id, b_id).unwrap();

        assert_eq!(result.pair.net, -2);
        assert_eq!(result.pair.revoked, 2);
        assert_eq!(result.pair.repos_hit, 1);
    }

    #[test]
    fn same_snapshot_pair_yields_zero_pair_stats() {
        let mut conn = open_conn();
        let snapshot = Snapshot {
            records: vec![
                record("TEAM", "repo-a", "acct-1", "Ada", Permission::Read),
                member_record("TEAM", "repo-a", "platform-eng", "acct-2", "Grace", Permission::Admin),
            ],
            repo_statuses: vec![ok_status("TEAM", "repo-a")],
        };
        let id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &snapshot, &[], &[]).unwrap();

        let result = get_roster_tree(&conn, id, id).unwrap();

        assert_eq!(
            result.pair,
            PairStats { added: 0, revoked: 0, changed: 0, escalations: 0, repos_hit: 0, net: 0 }
        );
    }

    #[test]
    fn repo_fetch_failed_on_comparison_side_renders_empty_roster_not_spurious_revokes() {
        let mut conn = open_conn();
        let a = Snapshot {
            records: vec![
                record("TEAM", "repo-a", "acct-1", "Ada", Permission::Read),
                record("TEAM", "repo-a", "acct-2", "Grace", Permission::Admin),
            ],
            repo_statuses: vec![ok_status("TEAM", "repo-a")],
        };
        // The permissions call for repo-a failed this run — no records collected for it, but
        // the repo was still discovered, so it gets a FetchFailed status, not an absent one.
        let b = Snapshot { records: vec![], repo_statuses: vec![fetch_failed_status("TEAM", "repo-a")] };

        let a_id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &a, &[], &[]).unwrap();
        let b_id = save_snapshot(&mut conn, "2026-01-02T00:00:00Z", &b, &[], &[]).unwrap();

        let result = get_roster_tree(&conn, a_id, b_id).unwrap();
        let repo = repo_node(&result.tree, "TEAM", "repo-a");

        assert!(repo.fetch_failed);
        // Without the fetch-failed override, Ada and Grace would show as two Revokes — the
        // whole point of this state is that we don't actually know that.
        assert!(repo.principals.is_empty());
        assert_eq!(repo.change_count, 0);
        assert_eq!(repo.read_count, 0);
        assert_eq!(repo.admin_count, 0);
        assert_eq!(repo.discovery_state, RepoDiscoveryState::Present);
    }

    #[test]
    fn repo_ok_with_zero_grants_is_distinct_from_a_fetch_failed_repo() {
        let mut conn = open_conn();
        let snapshot = Snapshot { records: vec![], repo_statuses: vec![ok_status("TEAM", "repo-a")] };
        let id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &snapshot, &[], &[]).unwrap();

        let result = get_roster_tree(&conn, id, id).unwrap();
        let repo = repo_node(&result.tree, "TEAM", "repo-a");

        assert!(!repo.fetch_failed);
        assert!(repo.principals.is_empty());
    }

    #[test]
    fn group_with_unresolvable_membership_is_flagged_on_its_own_row() {
        let mut conn = open_conn();
        let a = Snapshot::default();
        let b = Snapshot {
            records: vec![group_record("TEAM", "repo-a", "locked-group", "Locked Group", Permission::Read)],
            repo_statuses: vec![ok_status("TEAM", "repo-a")],
        };
        let membership_b = vec![membership_status("TEAM", "repo-a", "locked-group", false)];

        let a_id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &a, &[], &[]).unwrap();
        let b_id = save_snapshot(&mut conn, "2026-01-02T00:00:00Z", &b, &membership_b, &[]).unwrap();

        let result = get_roster_tree(&conn, a_id, b_id).unwrap();
        let repo = repo_node(&result.tree, "TEAM", "repo-a");

        assert_eq!(principal_entry(repo, "Locked Group").members_resolved, Some(false));
    }

    #[test]
    fn group_with_confirmed_membership_is_distinct_from_an_unresolved_one() {
        let mut conn = open_conn();
        let snapshot = Snapshot {
            records: vec![group_record("TEAM", "repo-a", "empty-group", "Empty Group", Permission::Read)],
            repo_statuses: vec![ok_status("TEAM", "repo-a")],
        };
        let membership = vec![membership_status("TEAM", "repo-a", "empty-group", true)];
        let id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &snapshot, &membership, &[]).unwrap();

        let result = get_roster_tree(&conn, id, id).unwrap();
        let repo = repo_node(&result.tree, "TEAM", "repo-a");

        // Both states render zero Member rows beneath the group — `members_resolved` is the
        // only thing distinguishing "confirmed empty" from "couldn't check".
        assert_eq!(principal_entry(repo, "Empty Group").members_resolved, Some(true));
        assert!(!repo.principals.iter().any(|p| matches!(p.access_type, AccessType::Member(_))));
    }

    #[test]
    fn direct_and_member_entries_never_carry_a_membership_resolution_fact() {
        let mut conn = open_conn();
        let snapshot = Snapshot {
            records: vec![
                record("TEAM", "repo-a", "acct-1", "Ada", Permission::Read),
                group_record("TEAM", "repo-a", "platform-eng", "Platform Engineering", Permission::Write),
                member_record("TEAM", "repo-a", "platform-eng", "acct-2", "Grace", Permission::Write),
            ],
            repo_statuses: vec![ok_status("TEAM", "repo-a")],
        };
        let membership = vec![membership_status("TEAM", "repo-a", "platform-eng", true)];
        let id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &snapshot, &membership, &[]).unwrap();

        let result = get_roster_tree(&conn, id, id).unwrap();
        let repo = repo_node(&result.tree, "TEAM", "repo-a");

        assert_eq!(principal_entry(repo, "Ada").members_resolved, None);
        assert_eq!(principal_entry(repo, "Grace").members_resolved, None);
        assert_eq!(
            principal_entry(repo, "Platform Engineering").members_resolved,
            Some(true)
        );
    }

    #[test]
    fn repo_level_and_project_level_grants_for_same_principal_are_two_separate_rows() {
        let mut conn = open_conn();
        let snapshot = Snapshot {
            records: vec![
                record("TEAM", "repo-a", "acct-1", "Ada", Permission::Read),
                project_record("TEAM", "repo-a", "acct-1", "Ada", Permission::Admin),
            ],
            repo_statuses: vec![ok_status("TEAM", "repo-a")],
        };
        let id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &snapshot, &[], &[]).unwrap();

        let result = get_roster_tree(&conn, id, id).unwrap();
        let repo = repo_node(&result.tree, "TEAM", "repo-a");

        assert_eq!(repo.principals.iter().filter(|p| p.principal.label == "Ada").count(), 2);
        let repo_level = principal_entry_with_scope(repo, "Ada", GrantScope::Repo);
        let project_level = principal_entry_with_scope(repo, "Ada", GrantScope::Project);
        assert_eq!(repo_level.permission, Permission::Read);
        assert_eq!(project_level.permission, Permission::Admin);
    }

    #[test]
    fn project_fetch_failure_appears_against_every_repo_in_the_project_without_affecting_repo_status() {
        let mut conn = open_conn();
        let snapshot = Snapshot {
            records: vec![
                record("TEAM", "repo-a", "acct-1", "Ada", Permission::Read),
                record("TEAM", "repo-b", "acct-2", "Grace", Permission::Write),
            ],
            repo_statuses: vec![ok_status("TEAM", "repo-a"), ok_status("TEAM", "repo-b")],
        };
        let id = save_snapshot(
            &mut conn,
            "2026-01-01T00:00:00Z",
            &snapshot,
            &[],
            &[project_fetch_failed_status("TEAM")],
        )
        .unwrap();

        let result = get_roster_tree(&conn, id, id).unwrap();
        let repo_a = repo_node(&result.tree, "TEAM", "repo-a");
        let repo_b = repo_node(&result.tree, "TEAM", "repo-b");

        // Both repos in the failed Project are flagged...
        assert!(repo_a.project_fetch_failed);
        assert!(repo_b.project_fetch_failed);
        // ...but each repo's own RepoStatus and data are unaffected: repo-level fetch succeeded,
        // so principals still show fully.
        assert_eq!(repo_a.status_b, Some(RepoStatus::Ok));
        assert!(!repo_a.fetch_failed);
        assert_eq!(repo_a.principals.len(), 1);
        assert_eq!(repo_b.principals.len(), 1);
    }

    #[test]
    fn repo_belonging_to_a_healthy_project_is_not_flagged() {
        let mut conn = open_conn();
        let snapshot =
            Snapshot { records: vec![], repo_statuses: vec![ok_status("TEAM", "repo-a")] };
        let id = save_snapshot(
            &mut conn,
            "2026-01-01T00:00:00Z",
            &snapshot,
            &[],
            &[project_ok_status("TEAM")],
        )
        .unwrap();

        let result = get_roster_tree(&conn, id, id).unwrap();
        let repo = repo_node(&result.tree, "TEAM", "repo-a");

        assert!(!repo.project_fetch_failed);
    }

    #[test]
    fn escalation_on_a_project_level_grant_produces_the_same_level_change_as_a_repo_level_one() {
        let mut conn = open_conn();
        let a = Snapshot {
            records: vec![project_record("TEAM", "repo-a", "acct-1", "Ada", Permission::Read)],
            repo_statuses: vec![ok_status("TEAM", "repo-a")],
        };
        let b = Snapshot {
            records: vec![project_record("TEAM", "repo-a", "acct-1", "Ada", Permission::Admin)],
            repo_statuses: vec![ok_status("TEAM", "repo-a")],
        };

        let a_id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &a, &[], &[]).unwrap();
        let b_id = save_snapshot(&mut conn, "2026-01-02T00:00:00Z", &b, &[], &[]).unwrap();

        let result = get_roster_tree(&conn, a_id, b_id).unwrap();
        let repo = repo_node(&result.tree, "TEAM", "repo-a");
        let ada = principal_entry_with_scope(repo, "Ada", GrantScope::Project);

        assert_eq!(
            ada.diff_status,
            DiffStatus::LevelChange {
                from: Permission::Read,
                to: Permission::Admin,
                kind: LevelChangeKind::Escalation,
            }
        );
        assert_eq!(repo.change_count, 1);
        assert_eq!(repo.admin_count, 1);
    }

    #[test]
    fn demotion_on_a_project_level_grant_produces_the_same_level_change_as_a_repo_level_one() {
        let mut conn = open_conn();
        let a = Snapshot {
            records: vec![project_record("TEAM", "repo-a", "acct-1", "Ada", Permission::Admin)],
            repo_statuses: vec![ok_status("TEAM", "repo-a")],
        };
        let b = Snapshot {
            records: vec![project_record("TEAM", "repo-a", "acct-1", "Ada", Permission::Write)],
            repo_statuses: vec![ok_status("TEAM", "repo-a")],
        };

        let a_id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &a, &[], &[]).unwrap();
        let b_id = save_snapshot(&mut conn, "2026-01-02T00:00:00Z", &b, &[], &[]).unwrap();

        let result = get_roster_tree(&conn, a_id, b_id).unwrap();
        let repo = repo_node(&result.tree, "TEAM", "repo-a");
        let ada = principal_entry_with_scope(repo, "Ada", GrantScope::Project);

        assert_eq!(
            ada.diff_status,
            DiffStatus::LevelChange {
                from: Permission::Admin,
                to: Permission::Write,
                kind: LevelChangeKind::Demotion,
            }
        );
        assert_eq!(repo.change_count, 1);
        assert_eq!(repo.write_count, 1);
    }

    #[test]
    fn project_level_grant_and_revoke_behave_identically_to_repo_level() {
        let mut conn = open_conn();
        let a = Snapshot {
            records: vec![project_record("TEAM", "repo-a", "acct-1", "Ada", Permission::Write)],
            repo_statuses: vec![ok_status("TEAM", "repo-a")],
        };
        let b = Snapshot {
            records: vec![project_record("TEAM", "repo-a", "acct-2", "Grace", Permission::Read)],
            repo_statuses: vec![ok_status("TEAM", "repo-a")],
        };

        let a_id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &a, &[], &[]).unwrap();
        let b_id = save_snapshot(&mut conn, "2026-01-02T00:00:00Z", &b, &[], &[]).unwrap();

        let result = get_roster_tree(&conn, a_id, b_id).unwrap();
        let repo = repo_node(&result.tree, "TEAM", "repo-a");

        assert_eq!(principal_entry(repo, "Ada").diff_status, DiffStatus::Revoke);
        assert_eq!(principal_entry(repo, "Grace").diff_status, DiffStatus::Grant);
        assert_eq!(repo.change_count, 2);
    }
}
