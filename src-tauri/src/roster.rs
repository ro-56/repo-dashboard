//! The Roster tree (ADR-0004): for a chosen Run pair, loads both Snapshots, runs the
//! existing diff engine (`diff.rs`, unmodified) for Grant/Revoke/Level-change entries, then
//! folds in the unchanged records so every Principal — changed or not — appears in the
//! tree. Computed fresh on every call; nothing here is persisted or cached.

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;
use serde::Serialize;

use crate::diff::{diff_snapshots, LevelChangeKind, RecordDiff, Snapshot};
use crate::model::{AccessType, Permission, PermissionRecord, RepoFetchStatus, RepoStatus};
use crate::storage::load_snapshot;

/// Mirrors `diff.rs`'s private record key exactly, so a record counted as "changed" by the
/// diff engine is never also folded in again as unchanged.
type RecordKey = (String, String, AccessType);

fn record_key(r: &PermissionRecord) -> RecordKey {
    (r.repo.clone(), r.principal.id.clone(), r.access_type.clone())
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
    /// The permission currently on display: Snapshot B's value for Grant/None/LevelChange,
    /// Snapshot A's value for Revoke (there is no B-side value for a revoked record).
    pub permission: Permission,
    pub diff_status: DiffStatus,
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
    pub principals: Vec<PrincipalEntry>,
    /// Counts reflect currently-held access (i.e. exclude Revoked entries) — a Grant, an
    /// unchanged record, and the "to" side of a Level change all count; a Revoke does not,
    /// since that Principal no longer holds that permission as of Snapshot B.
    pub read_count: usize,
    pub write_count: usize,
    pub admin_count: usize,
    /// Count of Grant + Revoke + Level-change entries in this repo. Zero whenever A and B
    /// are the same Snapshot.
    pub change_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectNode {
    pub repo_project: String,
    pub repos: Vec<RepoNode>,
}

pub type RosterTree = Vec<ProjectNode>;

fn repo_status_map(statuses: &[RepoFetchStatus]) -> HashMap<(String, String), RepoStatus> {
    statuses
        .iter()
        .map(|s| ((s.repo_project.clone(), s.repo.clone()), s.status))
        .collect()
}

fn build_roster_tree(a: &Snapshot, b: &Snapshot) -> RosterTree {
    let diff = diff_snapshots(a, b);

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
                        permission: r.permission,
                        diff_status: DiffStatus::Grant,
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
                        permission: r.permission,
                        diff_status: DiffStatus::Revoke,
                    },
                )
            }
            RecordDiff::LevelChange { repo_project, repo, principal, access_type, from, to, kind } => {
                changed_keys.insert((repo.clone(), principal.id.clone(), access_type.clone()));
                (
                    repo_project.clone(),
                    repo.clone(),
                    PrincipalEntry {
                        principal: principal.clone(),
                        access_type: access_type.clone(),
                        permission: *to,
                        diff_status: DiffStatus::LevelChange { from: *from, to: *to, kind: *kind },
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
            permission: r.permission,
            diff_status: DiffStatus::None,
        });
    }

    let status_a = repo_status_map(&a.repo_statuses);
    let status_b = repo_status_map(&b.repo_statuses);

    let mut repo_keys: HashSet<(String, String)> = HashSet::new();
    repo_keys.extend(status_a.keys().cloned());
    repo_keys.extend(status_b.keys().cloned());
    repo_keys.extend(by_repo.keys().cloned());

    let mut repos_by_project: HashMap<String, Vec<RepoNode>> = HashMap::new();

    for key in repo_keys {
        let (repo_project, repo) = key.clone();
        let mut principals = by_repo.remove(&key).unwrap_or_default();
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

        repos_by_project.entry(repo_project.clone()).or_default().push(RepoNode {
            repo_project,
            repo: repo.clone(),
            status_a: status_a.get(&key).copied(),
            status_b: status_b.get(&key).copied(),
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
/// unmodified, and returns the fully grouped, diff-annotated Roster tree. Selecting the same
/// Snapshot for both ids is a valid Run pair — every entry comes back with `DiffStatus::None`.
pub fn get_roster_tree(
    conn: &Connection,
    snapshot_a_id: i64,
    snapshot_b_id: i64,
) -> rusqlite::Result<RosterTree> {
    let a = load_snapshot(conn, snapshot_a_id)?;
    let b = load_snapshot(conn, snapshot_b_id)?;
    Ok(build_roster_tree(&a, &b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Principal;
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
            permission,
        }
    }

    fn ok_status(repo_project: &str, repo: &str) -> RepoFetchStatus {
        RepoFetchStatus {
            repo_project: repo_project.to_string(),
            repo: repo.to_string(),
            status: RepoStatus::Ok,
        }
    }

    fn repo_node<'a>(tree: &'a RosterTree, repo_project: &str, repo: &str) -> &'a RepoNode {
        tree.iter()
            .find(|p| p.repo_project == repo_project)
            .and_then(|p| p.repos.iter().find(|r| r.repo == repo))
            .expect("repo node present in tree")
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

        let a_id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &a, &[]).unwrap();
        let b_id = save_snapshot(&mut conn, "2026-01-02T00:00:00Z", &b, &[]).unwrap();

        let tree = get_roster_tree(&conn, a_id, b_id).unwrap();
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
        let id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &snapshot, &[]).unwrap();

        let tree = get_roster_tree(&conn, id, id).unwrap();
        let repo = repo_node(&tree, "TEAM", "repo-a");

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

        let a_id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &a, &[]).unwrap();
        let b_id = save_snapshot(&mut conn, "2026-01-02T00:00:00Z", &b, &[]).unwrap();

        let tree = get_roster_tree(&conn, a_id, b_id).unwrap();
        let repo_b = repo_node(&tree, "TEAM", "repo-b");

        assert_eq!(repo_b.status_a, Some(RepoStatus::Ok));
        assert_eq!(repo_b.status_b, None);
    }
}
