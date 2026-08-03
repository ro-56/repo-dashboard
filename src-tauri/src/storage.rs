//! SQLite persistence for `Snapshot`s. `init_schema` creates the four tables (`snapshots`,
//! `permission_records`, `repo_fetch_statuses`, `group_membership_statuses` — the last
//! persisted ahead of any consumer, per ADR-0003). `save_snapshot`/`load_snapshot` are pure
//! translations between the domain types and rows; `diff.rs` is never touched.

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::diff::Snapshot;
use crate::model::{
    AccessType, GrantScope, GroupMembershipStatus, Permission, PermissionRecord, Principal,
    ProjectFetchStatus, RepoFetchStatus, RepoStatus,
};

/// A Snapshot's identity for populating run selectors — never its full record set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotSummary {
    pub id: i64,
    pub run_at: String,
}

/// Every Snapshot's id + run_at, newest first.
pub fn list_snapshots(conn: &Connection) -> rusqlite::Result<Vec<SnapshotSummary>> {
    let mut stmt = conn.prepare("SELECT id, run_at FROM snapshots ORDER BY run_at DESC")?;
    let rows = stmt
        .query_map([], |row| {
            Ok(SnapshotSummary {
                id: row.get(0)?,
                run_at: row.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn init_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS snapshots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS permission_records (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            snapshot_id INTEGER NOT NULL REFERENCES snapshots(id),
            repo_project TEXT NOT NULL,
            repo TEXT NOT NULL,
            principal_id TEXT NOT NULL,
            principal_label TEXT NOT NULL,
            access_type TEXT NOT NULL,
            member_group_id TEXT,
            scope TEXT NOT NULL,
            permission TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_permission_records_snapshot_id
            ON permission_records(snapshot_id);

        CREATE TABLE IF NOT EXISTS repo_fetch_statuses (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            snapshot_id INTEGER NOT NULL REFERENCES snapshots(id),
            repo_project TEXT NOT NULL,
            repo TEXT NOT NULL,
            status TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_repo_fetch_statuses_snapshot_id
            ON repo_fetch_statuses(snapshot_id);

        CREATE TABLE IF NOT EXISTS project_fetch_statuses (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            snapshot_id INTEGER NOT NULL REFERENCES snapshots(id),
            project_key TEXT NOT NULL,
            status TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_project_fetch_statuses_snapshot_id
            ON project_fetch_statuses(snapshot_id);

        CREATE TABLE IF NOT EXISTS group_membership_statuses (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            snapshot_id INTEGER NOT NULL REFERENCES snapshots(id),
            repo_project TEXT NOT NULL,
            repo TEXT NOT NULL,
            group_id TEXT NOT NULL,
            members_resolved INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_group_membership_statuses_snapshot_id
            ON group_membership_statuses(snapshot_id);
        ",
    )?;

    // `CREATE TABLE IF NOT EXISTS` is a no-op on a database that already has
    // `permission_records` from before Project-scoped grants existed (ADR-0012), so it never
    // gains the `scope` column that save_snapshot/load_snapshot now require. Backfill it
    // explicitly: every such pre-existing row predates Project-scoped grants entirely, so
    // `repo` (see `encode_scope`) is the only correct value for it.
    let has_scope_column = conn
        .prepare("SELECT scope FROM permission_records LIMIT 0")
        .is_ok();
    if !has_scope_column {
        conn.execute(
            "ALTER TABLE permission_records ADD COLUMN scope TEXT NOT NULL DEFAULT 'repo'",
            [],
        )?;
    }

    Ok(())
}

/// Persists a full snapshot (all five tables) inside a single transaction, returning the
/// new `snapshots.id`. `run_at` is an RFC3339 timestamp string, passed through verbatim.
/// `project_statuses` is folded in here (rather than left to a separate
/// `save_project_fetch_statuses` call) so a Run's Snapshot can never be durably committed
/// while the overall Run is reported to the caller as failed.
pub fn save_snapshot(
    conn: &mut Connection,
    run_at: &str,
    snapshot: &Snapshot,
    group_membership_statuses: &[GroupMembershipStatus],
    project_statuses: &[ProjectFetchStatus],
) -> rusqlite::Result<i64> {
    let tx = conn.transaction()?;

    tx.execute("INSERT INTO snapshots (run_at) VALUES (?1)", params![run_at])?;
    let snapshot_id = tx.last_insert_rowid();

    {
        let mut stmt = tx.prepare(
            "INSERT INTO permission_records
                (snapshot_id, repo_project, repo, principal_id, principal_label,
                 access_type, member_group_id, scope, permission)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?;
        for r in &snapshot.records {
            let (access_type, member_group_id) = encode_access_type(&r.access_type);
            stmt.execute(params![
                snapshot_id,
                r.repo_project,
                r.repo,
                r.principal.id,
                r.principal.label,
                access_type,
                member_group_id,
                encode_scope(r.scope),
                permission_to_str(r.permission),
            ])?;
        }
    }

    {
        let mut stmt = tx.prepare(
            "INSERT INTO repo_fetch_statuses (snapshot_id, repo_project, repo, status)
             VALUES (?1, ?2, ?3, ?4)",
        )?;
        for s in &snapshot.repo_statuses {
            stmt.execute(params![
                snapshot_id,
                s.repo_project,
                s.repo,
                repo_status_to_str(s.status),
            ])?;
        }
    }

    {
        let mut stmt = tx.prepare(
            "INSERT INTO group_membership_statuses
                (snapshot_id, repo_project, repo, group_id, members_resolved)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for g in group_membership_statuses {
            stmt.execute(params![
                snapshot_id,
                g.repo_project,
                g.repo,
                g.group_id,
                g.members_resolved,
            ])?;
        }
    }

    {
        let mut stmt = tx.prepare(
            "INSERT INTO project_fetch_statuses (snapshot_id, project_key, status)
             VALUES (?1, ?2, ?3)",
        )?;
        for s in project_statuses {
            stmt.execute(params![snapshot_id, s.project_key, repo_status_to_str(s.status)])?;
        }
    }

    tx.commit()?;
    Ok(snapshot_id)
}

/// Reconstructs a `diff::Snapshot` (records + repo statuses) for the given snapshot id, in
/// the exact shape `diff_snapshots` consumes.
pub fn load_snapshot(conn: &Connection, snapshot_id: i64) -> rusqlite::Result<Snapshot> {
    let mut records_stmt = conn.prepare(
        "SELECT repo_project, repo, principal_id, principal_label, access_type,
                member_group_id, scope, permission
         FROM permission_records WHERE snapshot_id = ?1 ORDER BY id",
    )?;
    let records = records_stmt
        .query_map(params![snapshot_id], |row| {
            let access_type: String = row.get(4)?;
            let member_group_id: Option<String> = row.get(5)?;
            let scope: String = row.get(6)?;
            let permission: String = row.get(7)?;
            Ok(PermissionRecord {
                repo_project: row.get(0)?,
                repo: row.get(1)?,
                principal: Principal {
                    id: row.get(2)?,
                    label: row.get(3)?,
                },
                access_type: decode_access_type(&access_type, member_group_id),
                scope: decode_scope(&scope),
                permission: parse_permission(&permission),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut statuses_stmt = conn.prepare(
        "SELECT repo_project, repo, status FROM repo_fetch_statuses
         WHERE snapshot_id = ?1 ORDER BY id",
    )?;
    let repo_statuses = statuses_stmt
        .query_map(params![snapshot_id], |row| {
            let status: String = row.get(2)?;
            Ok(RepoFetchStatus {
                repo_project: row.get(0)?,
                repo: row.get(1)?,
                status: parse_repo_status(&status),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(Snapshot { records, repo_statuses })
}

/// Loads the `group_membership_statuses` rows for a snapshot. Not called by the diff engine
/// or any command yet (ADR-0003) — exists so the write path is provably lossless.
pub fn load_group_membership_statuses(
    conn: &Connection,
    snapshot_id: i64,
) -> rusqlite::Result<Vec<GroupMembershipStatus>> {
    let mut stmt = conn.prepare(
        "SELECT repo_project, repo, group_id, members_resolved
         FROM group_membership_statuses WHERE snapshot_id = ?1 ORDER BY id",
    )?;
    let rows = stmt
        .query_map(params![snapshot_id], |row| {
            Ok(GroupMembershipStatus {
                repo_project: row.get(0)?,
                repo: row.get(1)?,
                group_id: row.get(2)?,
                members_resolved: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Loads the `project_fetch_statuses` rows for a snapshot, mirroring
/// `load_group_membership_statuses`.
pub fn load_project_fetch_statuses(
    conn: &Connection,
    snapshot_id: i64,
) -> rusqlite::Result<Vec<ProjectFetchStatus>> {
    let mut stmt = conn.prepare(
        "SELECT project_key, status FROM project_fetch_statuses
         WHERE snapshot_id = ?1 ORDER BY id",
    )?;
    let rows = stmt
        .query_map(params![snapshot_id], |row| {
            let status: String = row.get(1)?;
            Ok(ProjectFetchStatus {
                project_key: row.get(0)?,
                status: parse_repo_status(&status),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Deleting a Snapshot that never existed is an error, not a silent no-op — the
/// frontend needs to know a delete request didn't correspond to anything real.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteSnapshotError {
    NotFound(i64),
    Storage(String),
}

/// Permanently removes a Snapshot and every row scoped to it (`permission_records`,
/// `repo_fetch_statuses`, `group_membership_statuses`, `project_fetch_statuses`) inside one
/// transaction. Child rows are deleted before the parent `snapshots` row since
/// `PRAGMA foreign_keys = ON` rejects a parent delete while children still reference it. Per
/// ADR-0011, this doesn't change `Snapshot`'s "immutable" meaning — deletion removes a
/// Snapshot's existence, not its content.
pub fn delete_snapshot(conn: &mut Connection, snapshot_id: i64) -> Result<(), DeleteSnapshotError> {
    fn storage_err(e: rusqlite::Error) -> DeleteSnapshotError {
        DeleteSnapshotError::Storage(e.to_string())
    }

    let tx = conn.transaction().map_err(storage_err)?;

    tx.execute("DELETE FROM permission_records WHERE snapshot_id = ?1", params![snapshot_id])
        .map_err(storage_err)?;
    tx.execute("DELETE FROM repo_fetch_statuses WHERE snapshot_id = ?1", params![snapshot_id])
        .map_err(storage_err)?;
    tx.execute("DELETE FROM group_membership_statuses WHERE snapshot_id = ?1", params![snapshot_id])
        .map_err(storage_err)?;
    tx.execute("DELETE FROM project_fetch_statuses WHERE snapshot_id = ?1", params![snapshot_id])
        .map_err(storage_err)?;

    let deleted = tx
        .execute("DELETE FROM snapshots WHERE id = ?1", params![snapshot_id])
        .map_err(storage_err)?;
    if deleted == 0 {
        return Err(DeleteSnapshotError::NotFound(snapshot_id));
    }

    tx.commit().map_err(storage_err)?;
    Ok(())
}

fn encode_access_type(access_type: &AccessType) -> (&'static str, Option<String>) {
    match access_type {
        AccessType::Direct => ("direct", None),
        AccessType::Group => ("group", None),
        AccessType::Member(group_id) => ("member", Some(group_id.clone())),
    }
}

fn decode_access_type(access_type: &str, member_group_id: Option<String>) -> AccessType {
    match access_type {
        "direct" => AccessType::Direct,
        "group" => AccessType::Group,
        "member" => AccessType::Member(
            member_group_id.expect("member access_type row missing member_group_id"),
        ),
        other => panic!("unknown access_type in storage: {other}"),
    }
}

fn encode_scope(scope: GrantScope) -> &'static str {
    match scope {
        GrantScope::Repo => "repo",
        GrantScope::Project => "project",
    }
}

fn decode_scope(s: &str) -> GrantScope {
    match s {
        "repo" => GrantScope::Repo,
        "project" => GrantScope::Project,
        other => panic!("unknown scope in storage: {other}"),
    }
}

fn permission_to_str(permission: Permission) -> &'static str {
    match permission {
        Permission::Read => "read",
        Permission::Write => "write",
        Permission::CreateRepo => "create-repo",
        Permission::Admin => "admin",
    }
}

fn parse_permission(s: &str) -> Permission {
    Permission::parse(s).unwrap_or_else(|| panic!("unknown permission in storage: {s}"))
}

fn repo_status_to_str(status: RepoStatus) -> &'static str {
    match status {
        RepoStatus::Ok => "ok",
        RepoStatus::FetchFailed => "fetch_failed",
    }
}

fn parse_repo_status(s: &str) -> RepoStatus {
    match s {
        "ok" => RepoStatus::Ok,
        "fetch_failed" => RepoStatus::FetchFailed,
        other => panic!("unknown repo status in storage: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::diff_snapshots;

    fn open_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        conn
    }

    fn sample_records() -> Vec<PermissionRecord> {
        vec![
            PermissionRecord {
                repo_project: "TEAM".to_string(),
                repo: "repo-a".to_string(),
                principal: Principal {
                    id: "acct-1".to_string(),
                    label: "Ada".to_string(),
                },
                access_type: AccessType::Direct,
                scope: GrantScope::Repo,
                permission: Permission::Read,
            },
            PermissionRecord {
                repo_project: "TEAM".to_string(),
                repo: "repo-a".to_string(),
                principal: Principal {
                    id: "platform-eng".to_string(),
                    label: "Platform Engineering".to_string(),
                },
                access_type: AccessType::Group,
                scope: GrantScope::Project,
                permission: Permission::Write,
            },
            PermissionRecord {
                repo_project: "TEAM".to_string(),
                repo: "repo-a".to_string(),
                principal: Principal {
                    id: "acct-2".to_string(),
                    label: "Grace".to_string(),
                },
                access_type: AccessType::Member("platform-eng".to_string()),
                scope: GrantScope::Repo,
                permission: Permission::Write,
            },
        ]
    }

    #[test]
    fn round_trips_direct_group_and_member_records_losslessly() {
        let mut conn = open_conn();
        let snapshot = Snapshot {
            records: sample_records(),
            repo_statuses: vec![],
        };

        let id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &snapshot, &[], &[]).unwrap();
        let loaded = load_snapshot(&conn, id).unwrap();

        assert_eq!(loaded, snapshot);
    }

    #[test]
    fn create_repo_permission_round_trips_through_save_and_load() {
        let mut conn = open_conn();
        let snapshot = Snapshot {
            records: vec![PermissionRecord {
                repo_project: "TEAM".to_string(),
                repo: "repo-a".to_string(),
                principal: Principal { id: "acct-1".to_string(), label: "Ada".to_string() },
                access_type: AccessType::Direct,
                scope: GrantScope::Project,
                permission: Permission::CreateRepo,
            }],
            repo_statuses: vec![],
        };

        let id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &snapshot, &[], &[]).unwrap();
        let loaded = load_snapshot(&conn, id).unwrap();

        assert_eq!(loaded, snapshot);
        assert_eq!(loaded.records[0].permission, Permission::CreateRepo);
    }

    #[test]
    fn scope_round_trips_independently_of_access_type() {
        // sample_records() already mixes scope across access types (Direct=Repo, Group=Project,
        // Member=Repo); this test pins down the orthogonal case explicitly: the same access_type
        // at both scope values must round-trip to distinct, correct scopes.
        let mut conn = open_conn();
        let snapshot = Snapshot {
            records: vec![
                PermissionRecord {
                    repo_project: "TEAM".to_string(),
                    repo: "repo-a".to_string(),
                    principal: Principal { id: "acct-1".to_string(), label: "Ada".to_string() },
                    access_type: AccessType::Direct,
                    scope: GrantScope::Repo,
                    permission: Permission::Read,
                },
                PermissionRecord {
                    repo_project: "TEAM".to_string(),
                    repo: "repo-a".to_string(),
                    principal: Principal { id: "acct-1".to_string(), label: "Ada".to_string() },
                    access_type: AccessType::Direct,
                    scope: GrantScope::Project,
                    permission: Permission::Read,
                },
            ],
            repo_statuses: vec![],
        };

        let id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &snapshot, &[], &[]).unwrap();
        let loaded = load_snapshot(&conn, id).unwrap();

        assert_eq!(loaded, snapshot);
        assert_eq!(loaded.records[0].scope, GrantScope::Repo);
        assert_eq!(loaded.records[1].scope, GrantScope::Project);
    }

    #[test]
    fn project_fetch_status_round_trips_through_save_and_load() {
        let mut conn = open_conn();
        let statuses = vec![
            ProjectFetchStatus { project_key: "TEAM".to_string(), status: RepoStatus::Ok },
            ProjectFetchStatus { project_key: "OTHER".to_string(), status: RepoStatus::FetchFailed },
        ];
        let id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &Snapshot::default(), &[], &statuses)
            .unwrap();

        let loaded = load_project_fetch_statuses(&conn, id).unwrap();
        assert_eq!(loaded, statuses);
    }

    #[test]
    fn project_fetch_statuses_are_scoped_to_their_own_snapshot() {
        let mut conn = open_conn();
        save_snapshot(
            &mut conn,
            "2026-01-01T00:00:00Z",
            &Snapshot::default(),
            &[],
            &[ProjectFetchStatus { project_key: "TEAM".to_string(), status: RepoStatus::Ok }],
        )
        .unwrap();
        let second_id =
            save_snapshot(&mut conn, "2026-01-02T00:00:00Z", &Snapshot::default(), &[], &[]).unwrap();

        assert!(load_project_fetch_statuses(&conn, second_id).unwrap().is_empty());
    }

    #[test]
    fn round_trips_repo_fetch_statuses_including_failures() {
        let mut conn = open_conn();
        let snapshot = Snapshot {
            records: vec![],
            repo_statuses: vec![
                RepoFetchStatus {
                    repo_project: "TEAM".to_string(),
                    repo: "repo-a".to_string(),
                    status: RepoStatus::Ok,
                },
                RepoFetchStatus {
                    repo_project: "TEAM".to_string(),
                    repo: "repo-b".to_string(),
                    status: RepoStatus::FetchFailed,
                },
            ],
        };

        let id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &snapshot, &[], &[]).unwrap();
        let loaded = load_snapshot(&conn, id).unwrap();

        assert_eq!(loaded, snapshot);
    }

    #[test]
    fn round_trips_group_membership_statuses() {
        let mut conn = open_conn();
        let statuses = vec![
            GroupMembershipStatus {
                repo_project: "TEAM".to_string(),
                repo: "repo-a".to_string(),
                group_id: "platform-eng".to_string(),
                members_resolved: true,
            },
            GroupMembershipStatus {
                repo_project: "TEAM".to_string(),
                repo: "repo-a".to_string(),
                group_id: "unreachable-group".to_string(),
                members_resolved: false,
            },
        ];

        let id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &Snapshot::default(), &statuses, &[]).unwrap();
        let loaded = load_group_membership_statuses(&conn, id).unwrap();

        assert_eq!(loaded, statuses);
    }

    #[test]
    fn save_snapshot_writes_all_four_tables_together() {
        let mut conn = open_conn();
        let snapshot = Snapshot {
            records: sample_records(),
            repo_statuses: vec![RepoFetchStatus {
                repo_project: "TEAM".to_string(),
                repo: "repo-a".to_string(),
                status: RepoStatus::Ok,
            }],
        };
        let group_statuses = vec![GroupMembershipStatus {
            repo_project: "TEAM".to_string(),
            repo: "repo-a".to_string(),
            group_id: "platform-eng".to_string(),
            members_resolved: true,
        }];

        let id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &snapshot, &group_statuses, &[]).unwrap();

        let permission_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM permission_records WHERE snapshot_id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap();
        let repo_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM repo_fetch_statuses WHERE snapshot_id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap();
        let group_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM group_membership_statuses WHERE snapshot_id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(permission_count, 3);
        assert_eq!(repo_count, 1);
        assert_eq!(group_count, 1);
    }

    #[test]
    fn diffing_persisted_snapshots_matches_diffing_in_memory_fixtures() {
        let mut b_records = sample_records();
        b_records[0].permission = Permission::Admin; // escalate Ada's direct grant

        let a = Snapshot {
            records: sample_records(),
            repo_statuses: vec![
                RepoFetchStatus {
                    repo_project: "TEAM".to_string(),
                    repo: "repo-a".to_string(),
                    status: RepoStatus::Ok,
                },
                RepoFetchStatus {
                    repo_project: "TEAM".to_string(),
                    repo: "repo-b".to_string(),
                    status: RepoStatus::Ok,
                },
            ],
        };
        let b = Snapshot {
            records: b_records,
            repo_statuses: vec![RepoFetchStatus {
                repo_project: "TEAM".to_string(),
                repo: "repo-a".to_string(),
                status: RepoStatus::FetchFailed,
            }],
        };

        let expected = diff_snapshots(&a, &b);

        let mut conn = open_conn();
        let a_id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &a, &[], &[]).unwrap();
        let b_id = save_snapshot(&mut conn, "2026-01-02T00:00:00Z", &b, &[], &[]).unwrap();

        let a_loaded = load_snapshot(&conn, a_id).unwrap();
        let b_loaded = load_snapshot(&conn, b_id).unwrap();

        let actual = diff_snapshots(&a_loaded, &b_loaded);
        assert_eq!(actual, expected);
    }

    #[test]
    fn snapshot_ids_are_distinct_and_increasing_across_saves() {
        let mut conn = open_conn();
        let first = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &Snapshot::default(), &[], &[]).unwrap();
        let second = save_snapshot(&mut conn, "2026-01-02T00:00:00Z", &Snapshot::default(), &[], &[]).unwrap();
        assert!(second > first);
    }

    #[test]
    fn delete_snapshot_removes_the_snapshot_and_all_associated_rows_but_spares_other_snapshots() {
        let mut conn = open_conn();
        let survivor = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &Snapshot::default(), &[], &[]).unwrap();

        let snapshot = Snapshot {
            records: sample_records(),
            repo_statuses: vec![RepoFetchStatus {
                repo_project: "TEAM".to_string(),
                repo: "repo-a".to_string(),
                status: RepoStatus::Ok,
            }],
        };
        let group_statuses = vec![GroupMembershipStatus {
            repo_project: "TEAM".to_string(),
            repo: "repo-a".to_string(),
            group_id: "platform-eng".to_string(),
            members_resolved: true,
        }];
        let id = save_snapshot(
            &mut conn,
            "2026-01-02T00:00:00Z",
            &snapshot,
            &group_statuses,
            &[ProjectFetchStatus { project_key: "TEAM".to_string(), status: RepoStatus::Ok }],
        )
        .unwrap();

        delete_snapshot(&mut conn, id).unwrap();

        let snapshot_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM snapshots WHERE id = ?1", params![id], |row| row.get(0))
            .unwrap();
        let permission_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM permission_records WHERE snapshot_id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap();
        let repo_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM repo_fetch_statuses WHERE snapshot_id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap();
        let group_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM group_membership_statuses WHERE snapshot_id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(snapshot_count, 0);
        assert_eq!(permission_count, 0);
        assert_eq!(repo_count, 0);
        assert_eq!(group_count, 0);
        assert!(load_project_fetch_statuses(&conn, id).unwrap().is_empty());

        let survivor_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM snapshots WHERE id = ?1", params![survivor], |row| row.get(0))
            .unwrap();
        assert_eq!(survivor_count, 1);
    }

    #[test]
    fn delete_snapshot_excludes_it_from_list_snapshots() {
        let mut conn = open_conn();
        let kept = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &Snapshot::default(), &[], &[]).unwrap();
        let deleted = save_snapshot(&mut conn, "2026-01-02T00:00:00Z", &Snapshot::default(), &[], &[]).unwrap();

        delete_snapshot(&mut conn, deleted).unwrap();

        let listed = list_snapshots(&conn).unwrap();
        assert_eq!(
            listed,
            vec![SnapshotSummary { id: kept, run_at: "2026-01-01T00:00:00Z".to_string() }]
        );
    }

    #[test]
    fn delete_snapshot_on_unknown_id_errors() {
        let mut conn = open_conn();
        let result = delete_snapshot(&mut conn, 9999);
        assert_eq!(result, Err(DeleteSnapshotError::NotFound(9999)));
    }

    #[test]
    fn list_snapshots_returns_every_snapshot_newest_first() {
        let mut conn = open_conn();
        let first = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &Snapshot::default(), &[], &[]).unwrap();
        let second = save_snapshot(&mut conn, "2026-01-02T00:00:00Z", &Snapshot::default(), &[], &[]).unwrap();

        let listed = list_snapshots(&conn).unwrap();

        assert_eq!(
            listed,
            vec![
                SnapshotSummary { id: second, run_at: "2026-01-02T00:00:00Z".to_string() },
                SnapshotSummary { id: first, run_at: "2026-01-01T00:00:00Z".to_string() },
            ]
        );
    }
}

#[cfg(test)]
mod legacy_schema_upgrade_tests {
    use super::*;

    fn open_pre_pd28_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;
            CREATE TABLE snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                run_at TEXT NOT NULL
            );
            CREATE TABLE permission_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                snapshot_id INTEGER NOT NULL REFERENCES snapshots(id),
                repo_project TEXT NOT NULL,
                repo TEXT NOT NULL,
                principal_id TEXT NOT NULL,
                principal_label TEXT NOT NULL,
                access_type TEXT NOT NULL,
                member_group_id TEXT,
                permission TEXT NOT NULL
            );
            CREATE TABLE repo_fetch_statuses (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                snapshot_id INTEGER NOT NULL REFERENCES snapshots(id),
                repo_project TEXT NOT NULL,
                repo TEXT NOT NULL,
                status TEXT NOT NULL
            );
            CREATE TABLE group_membership_statuses (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                snapshot_id INTEGER NOT NULL REFERENCES snapshots(id),
                repo_project TEXT NOT NULL,
                repo TEXT NOT NULL,
                group_id TEXT NOT NULL,
                members_resolved INTEGER NOT NULL
            );
            INSERT INTO snapshots (id, run_at) VALUES (1, '2026-01-01T00:00:00Z');
            INSERT INTO permission_records
                (snapshot_id, repo_project, repo, principal_id, principal_label,
                 access_type, member_group_id, permission)
             VALUES (1, 'TEAM', 'repo-a', 'acct-1', 'Ada', 'direct', NULL, 'read');
            ",
        )
        .unwrap();
        conn
    }

    #[test]
    fn init_schema_backfills_scope_on_a_pre_pd28_database() {
        let mut conn = open_pre_pd28_conn();
        init_schema(&conn).unwrap();

        let snapshot = Snapshot { records: vec![], repo_statuses: vec![] };
        save_snapshot(&mut conn, "2026-07-30T00:00:00Z", &snapshot, &[], &[])
            .expect("save_snapshot should succeed against an upgraded legacy schema");

        let scope: String = conn
            .query_row(
                "SELECT scope FROM permission_records WHERE principal_id = 'acct-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(scope, "repo");
    }

    #[test]
    fn init_schema_is_idempotent_on_a_pre_pd28_database() {
        let conn = open_pre_pd28_conn();
        init_schema(&conn).unwrap();
        init_schema(&conn).unwrap();
    }

    // Regression test for the blank-dashboard-on-relaunch bug: get_roster_tree calls
    // load_snapshot on startup for whatever snapshots already exist, so a pre-PD28 snapshot
    // must stay loadable after migration, not just writable by new saves.
    #[test]
    fn load_snapshot_reads_a_pre_pd28_row_after_migration() {
        let conn = open_pre_pd28_conn();
        init_schema(&conn).unwrap();

        let snapshot = load_snapshot(&conn, 1).expect("loading a pre-PD28 snapshot should not error");
        assert_eq!(snapshot.records.len(), 1);
        assert_eq!(snapshot.records[0].scope, GrantScope::Repo);
    }
}
