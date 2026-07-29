//! SQLite persistence for `Snapshot`s. `init_schema` creates the four tables (`snapshots`,
//! `permission_records`, `repo_fetch_statuses`, `group_membership_statuses` — the last
//! persisted ahead of any consumer, per ADR-0003). `save_snapshot`/`load_snapshot` are pure
//! translations between the PD-1–PD-4 domain types and rows; `diff.rs` is never touched.

use rusqlite::{params, Connection};

use crate::diff::Snapshot;
use crate::model::{
    AccessType, GroupMembershipStatus, Permission, PermissionRecord, Principal, RepoFetchStatus,
    RepoStatus,
};

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
    )
}

/// Persists a full snapshot (all four tables) inside a single transaction, returning the
/// new `snapshots.id`. `run_at` is an RFC3339 timestamp string, passed through verbatim.
pub fn save_snapshot(
    conn: &mut Connection,
    run_at: &str,
    snapshot: &Snapshot,
    group_membership_statuses: &[GroupMembershipStatus],
) -> rusqlite::Result<i64> {
    let tx = conn.transaction()?;

    tx.execute("INSERT INTO snapshots (run_at) VALUES (?1)", params![run_at])?;
    let snapshot_id = tx.last_insert_rowid();

    {
        let mut stmt = tx.prepare(
            "INSERT INTO permission_records
                (snapshot_id, repo_project, repo, principal_id, principal_label,
                 access_type, member_group_id, permission)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
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

    tx.commit()?;
    Ok(snapshot_id)
}

/// Reconstructs a `diff::Snapshot` (records + repo statuses) for the given snapshot id, in
/// the exact shape `diff_snapshots` consumes.
pub fn load_snapshot(conn: &Connection, snapshot_id: i64) -> rusqlite::Result<Snapshot> {
    let mut records_stmt = conn.prepare(
        "SELECT repo_project, repo, principal_id, principal_label, access_type,
                member_group_id, permission
         FROM permission_records WHERE snapshot_id = ?1 ORDER BY id",
    )?;
    let records = records_stmt
        .query_map(params![snapshot_id], |row| {
            let access_type: String = row.get(4)?;
            let member_group_id: Option<String> = row.get(5)?;
            let permission: String = row.get(6)?;
            Ok(PermissionRecord {
                repo_project: row.get(0)?,
                repo: row.get(1)?,
                principal: Principal {
                    id: row.get(2)?,
                    label: row.get(3)?,
                },
                access_type: decode_access_type(&access_type, member_group_id),
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

fn permission_to_str(permission: Permission) -> &'static str {
    match permission {
        Permission::Read => "read",
        Permission::Write => "write",
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

        let id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &snapshot, &[]).unwrap();
        let loaded = load_snapshot(&conn, id).unwrap();

        assert_eq!(loaded, snapshot);
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

        let id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &snapshot, &[]).unwrap();
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

        let id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &Snapshot::default(), &statuses).unwrap();
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

        let id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &snapshot, &group_statuses).unwrap();

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
        let a_id = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &a, &[]).unwrap();
        let b_id = save_snapshot(&mut conn, "2026-01-02T00:00:00Z", &b, &[]).unwrap();

        let a_loaded = load_snapshot(&conn, a_id).unwrap();
        let b_loaded = load_snapshot(&conn, b_id).unwrap();

        let actual = diff_snapshots(&a_loaded, &b_loaded);
        assert_eq!(actual, expected);
    }

    #[test]
    fn snapshot_ids_are_distinct_and_increasing_across_saves() {
        let mut conn = open_conn();
        let first = save_snapshot(&mut conn, "2026-01-01T00:00:00Z", &Snapshot::default(), &[]).unwrap();
        let second = save_snapshot(&mut conn, "2026-01-02T00:00:00Z", &Snapshot::default(), &[]).unwrap();
        assert!(second > first);
    }
}
