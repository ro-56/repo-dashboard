use std::collections::{BTreeSet, HashMap};

use crate::model::{AccessType, Permission, PermissionRecord, Principal, RepoFetchStatus, RepoStatus};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Snapshot {
    pub records: Vec<PermissionRecord>,
    pub repo_statuses: Vec<RepoFetchStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelChangeKind {
    Escalation,
    Demotion,
}

/// A diff entry for one grant-source record, keyed on `(repo, principal_id, access_type)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordDiff {
    Grant(PermissionRecord),
    Revoke(PermissionRecord),
    LevelChange {
        repo_project: String,
        repo: String,
        principal: Principal,
        access_type: AccessType,
        from: Permission,
        to: Permission,
        kind: LevelChangeKind,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    A,
    B,
}

/// A repo-level diff entry. `AbsentFromDiscovery` (no status row at all on one side) is
/// always reported distinctly from `FetchFailed` (discovered on both sides, but the
/// permissions fetch errored on one) — the two must never be conflated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoDiff {
    AbsentFromDiscovery {
        repo_project: String,
        repo: String,
        absent_in: Side,
    },
    FetchFailed {
        repo_project: String,
        repo: String,
        failed_in: Side,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffResult {
    pub records: Vec<RecordDiff>,
    pub repos: Vec<RepoDiff>,
}

type RecordKey = (String, String, AccessType);

fn record_key(r: &PermissionRecord) -> RecordKey {
    (r.repo.clone(), r.principal.id.clone(), r.access_type.clone())
}

fn diff_records(a: &[PermissionRecord], b: &[PermissionRecord]) -> Vec<RecordDiff> {
    let a_map: HashMap<RecordKey, &PermissionRecord> =
        a.iter().map(|r| (record_key(r), r)).collect();
    let b_map: HashMap<RecordKey, &PermissionRecord> =
        b.iter().map(|r| (record_key(r), r)).collect();

    let keys: BTreeSet<RecordKey> = a_map.keys().cloned().chain(b_map.keys().cloned()).collect();

    keys.into_iter()
        .filter_map(|key| match (a_map.get(&key), b_map.get(&key)) {
            (None, Some(rb)) => Some(RecordDiff::Grant((*rb).clone())),
            (Some(ra), None) => Some(RecordDiff::Revoke((*ra).clone())),
            (Some(ra), Some(rb)) => {
                if ra.permission == rb.permission {
                    None
                } else {
                    let kind = if rb.permission > ra.permission {
                        LevelChangeKind::Escalation
                    } else {
                        LevelChangeKind::Demotion
                    };
                    Some(RecordDiff::LevelChange {
                        repo_project: rb.repo_project.clone(),
                        repo: rb.repo.clone(),
                        principal: rb.principal.clone(),
                        access_type: rb.access_type.clone(),
                        from: ra.permission,
                        to: rb.permission,
                        kind,
                    })
                }
            }
            (None, None) => unreachable!("key only exists because it came from a_map or b_map"),
        })
        .collect()
}

fn diff_repo_statuses(a: &[RepoFetchStatus], b: &[RepoFetchStatus]) -> Vec<RepoDiff> {
    type RepoKey = (String, String);

    let a_map: HashMap<RepoKey, RepoStatus> = a
        .iter()
        .map(|s| ((s.repo_project.clone(), s.repo.clone()), s.status))
        .collect();
    let b_map: HashMap<RepoKey, RepoStatus> = b
        .iter()
        .map(|s| ((s.repo_project.clone(), s.repo.clone()), s.status))
        .collect();

    let keys: BTreeSet<RepoKey> = a_map.keys().cloned().chain(b_map.keys().cloned()).collect();

    keys.into_iter()
        .filter_map(|key| {
            let a_status = a_map.get(&key);
            let b_status = b_map.get(&key);
            let (repo_project, repo) = key;
            match (a_status, b_status) {
                (None, Some(_)) => Some(RepoDiff::AbsentFromDiscovery {
                    repo_project,
                    repo,
                    absent_in: Side::A,
                }),
                (Some(_), None) => Some(RepoDiff::AbsentFromDiscovery {
                    repo_project,
                    repo,
                    absent_in: Side::B,
                }),
                (Some(RepoStatus::FetchFailed), Some(RepoStatus::Ok)) => Some(RepoDiff::FetchFailed {
                    repo_project,
                    repo,
                    failed_in: Side::A,
                }),
                (Some(RepoStatus::Ok), Some(RepoStatus::FetchFailed)) => Some(RepoDiff::FetchFailed {
                    repo_project,
                    repo,
                    failed_in: Side::B,
                }),
                (Some(RepoStatus::Ok), Some(RepoStatus::Ok)) => None,
                (Some(RepoStatus::FetchFailed), Some(RepoStatus::FetchFailed)) => None,
                (None, None) => unreachable!("key only exists because it came from a_map or b_map"),
            }
        })
        .collect()
}

/// Compares two snapshots and returns record-level and repo-level diff entries.
pub fn diff_snapshots(a: &Snapshot, b: &Snapshot) -> DiffResult {
    DiffResult {
        records: diff_records(&a.records, &b.records),
        repos: diff_repo_statuses(&a.repo_statuses, &b.repo_statuses),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(account_id: &str, label: &str, permission: Permission) -> PermissionRecord {
        PermissionRecord {
            repo_project: "TEAM".to_string(),
            repo: "repo-a".to_string(),
            principal: Principal {
                id: account_id.to_string(),
                label: label.to_string(),
            },
            access_type: AccessType::Direct,
            permission,
        }
    }

    fn ok_status(repo: &str) -> RepoFetchStatus {
        RepoFetchStatus {
            repo_project: "TEAM".to_string(),
            repo: repo.to_string(),
            status: RepoStatus::Ok,
        }
    }

    #[test]
    fn grant_when_record_only_in_b() {
        let a = Snapshot::default();
        let b = Snapshot {
            records: vec![record("acct-1", "Ada", Permission::Read)],
            repo_statuses: vec![],
        };

        let diff = diff_snapshots(&a, &b);
        assert_eq!(diff.records, vec![RecordDiff::Grant(record("acct-1", "Ada", Permission::Read))]);
    }

    #[test]
    fn revoke_when_record_only_in_a() {
        let a = Snapshot {
            records: vec![record("acct-1", "Ada", Permission::Read)],
            repo_statuses: vec![],
        };
        let b = Snapshot::default();

        let diff = diff_snapshots(&a, &b);
        assert_eq!(diff.records, vec![RecordDiff::Revoke(record("acct-1", "Ada", Permission::Read))]);
    }

    #[test]
    fn escalation_when_level_rises() {
        let a = Snapshot {
            records: vec![record("acct-1", "Ada", Permission::Read)],
            repo_statuses: vec![],
        };
        let b = Snapshot {
            records: vec![record("acct-1", "Ada", Permission::Admin)],
            repo_statuses: vec![],
        };

        let diff = diff_snapshots(&a, &b);
        assert_eq!(
            diff.records,
            vec![RecordDiff::LevelChange {
                repo_project: "TEAM".to_string(),
                repo: "repo-a".to_string(),
                principal: Principal {
                    id: "acct-1".to_string(),
                    label: "Ada".to_string(),
                },
                access_type: AccessType::Direct,
                from: Permission::Read,
                to: Permission::Admin,
                kind: LevelChangeKind::Escalation,
            }]
        );
    }

    #[test]
    fn demotion_when_level_falls() {
        let a = Snapshot {
            records: vec![record("acct-1", "Ada", Permission::Admin)],
            repo_statuses: vec![],
        };
        let b = Snapshot {
            records: vec![record("acct-1", "Ada", Permission::Write)],
            repo_statuses: vec![],
        };

        let diff = diff_snapshots(&a, &b);
        assert_eq!(
            diff.records,
            vec![RecordDiff::LevelChange {
                repo_project: "TEAM".to_string(),
                repo: "repo-a".to_string(),
                principal: Principal {
                    id: "acct-1".to_string(),
                    label: "Ada".to_string(),
                },
                access_type: AccessType::Direct,
                from: Permission::Admin,
                to: Permission::Write,
                kind: LevelChangeKind::Demotion,
            }]
        );
    }

    #[test]
    fn no_diff_when_record_unchanged() {
        let a = Snapshot {
            records: vec![record("acct-1", "Ada", Permission::Write)],
            repo_statuses: vec![],
        };
        let b = Snapshot {
            records: vec![record("acct-1", "Ada", Permission::Write)],
            repo_statuses: vec![],
        };

        let diff = diff_snapshots(&a, &b);
        assert!(diff.records.is_empty());
    }

    #[test]
    fn no_diff_when_only_label_changes() {
        let a = Snapshot {
            records: vec![record("acct-1", "Ada Lovelace", Permission::Write)],
            repo_statuses: vec![],
        };
        let b = Snapshot {
            records: vec![record("acct-1", "Ada L.", Permission::Write)],
            repo_statuses: vec![],
        };

        let diff = diff_snapshots(&a, &b);
        assert!(diff.records.is_empty());
    }

    #[test]
    fn repo_fetch_failed_in_one_run_is_distinct_from_absent_from_discovery() {
        let a = Snapshot {
            records: vec![],
            repo_statuses: vec![ok_status("repo-a"), ok_status("repo-b")],
        };
        let b = Snapshot {
            records: vec![],
            repo_statuses: vec![
                RepoFetchStatus {
                    repo_project: "TEAM".to_string(),
                    repo: "repo-a".to_string(),
                    status: RepoStatus::FetchFailed,
                },
                // repo-b entirely absent from this run's discovery
            ],
        };

        let diff = diff_snapshots(&a, &b);
        assert_eq!(
            diff.repos,
            vec![
                RepoDiff::FetchFailed {
                    repo_project: "TEAM".to_string(),
                    repo: "repo-a".to_string(),
                    failed_in: Side::B,
                },
                RepoDiff::AbsentFromDiscovery {
                    repo_project: "TEAM".to_string(),
                    repo: "repo-b".to_string(),
                    absent_in: Side::B,
                },
            ]
        );
    }

    #[test]
    fn no_repo_diff_when_both_ok() {
        let a = Snapshot {
            records: vec![],
            repo_statuses: vec![ok_status("repo-a")],
        };
        let b = Snapshot {
            records: vec![],
            repo_statuses: vec![ok_status("repo-a")],
        };

        let diff = diff_snapshots(&a, &b);
        assert!(diff.repos.is_empty());
    }
}
