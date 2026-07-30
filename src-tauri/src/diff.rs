use std::collections::{BTreeSet, HashMap};

use serde::Serialize;

use crate::model::{
    AccessType, GrantScope, Permission, PermissionRecord, Principal, RepoFetchStatus, RepoStatus,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Snapshot {
    pub records: Vec<PermissionRecord>,
    pub repo_statuses: Vec<RepoFetchStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LevelChangeKind {
    Escalation,
    Demotion,
}

/// A diff entry for one grant-source record, keyed on `(repo, principal_id, scope, access_type)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordDiff {
    Grant(PermissionRecord),
    Revoke(PermissionRecord),
    LevelChange {
        repo_project: String,
        repo: String,
        principal: Principal,
        access_type: AccessType,
        scope: GrantScope,
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

type RecordKey = (String, String, GrantScope, AccessType);

fn record_key(r: &PermissionRecord) -> RecordKey {
    (r.repo.clone(), r.principal.id.clone(), r.scope, r.access_type.clone())
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
                        scope: rb.scope,
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
            scope: GrantScope::Repo,
            permission,
        }
    }

    fn project_record(account_id: &str, label: &str, permission: Permission) -> PermissionRecord {
        PermissionRecord {
            scope: GrantScope::Project,
            ..record(account_id, label, permission)
        }
    }

    fn group_record(group_id: &str, label: &str, permission: Permission) -> PermissionRecord {
        PermissionRecord {
            repo_project: "TEAM".to_string(),
            repo: "repo-a".to_string(),
            principal: Principal {
                id: group_id.to_string(),
                label: label.to_string(),
            },
            access_type: AccessType::Group,
            scope: GrantScope::Repo,
            permission,
        }
    }

    fn member_record(group_id: &str, account_id: &str, label: &str, permission: Permission) -> PermissionRecord {
        PermissionRecord {
            repo_project: "TEAM".to_string(),
            repo: "repo-a".to_string(),
            principal: Principal {
                id: account_id.to_string(),
                label: label.to_string(),
            },
            access_type: AccessType::Member(group_id.to_string()),
            scope: GrantScope::Repo,
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
                scope: GrantScope::Repo,
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
                scope: GrantScope::Repo,
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
    fn grant_for_group_own_grant_when_record_only_in_b() {
        let a = Snapshot::default();
        let b = Snapshot {
            records: vec![group_record("platform-eng", "Platform Engineering", Permission::Read)],
            repo_statuses: vec![],
        };

        let diff = diff_snapshots(&a, &b);
        assert_eq!(
            diff.records,
            vec![RecordDiff::Grant(group_record(
                "platform-eng",
                "Platform Engineering",
                Permission::Read
            ))]
        );
    }

    #[test]
    fn revoke_for_group_own_grant_when_record_only_in_a() {
        let a = Snapshot {
            records: vec![group_record("platform-eng", "Platform Engineering", Permission::Read)],
            repo_statuses: vec![],
        };
        let b = Snapshot::default();

        let diff = diff_snapshots(&a, &b);
        assert_eq!(
            diff.records,
            vec![RecordDiff::Revoke(group_record(
                "platform-eng",
                "Platform Engineering",
                Permission::Read
            ))]
        );
    }

    #[test]
    fn escalation_when_group_own_grant_level_rises() {
        let a = Snapshot {
            records: vec![group_record("platform-eng", "Platform Engineering", Permission::Read)],
            repo_statuses: vec![],
        };
        let b = Snapshot {
            records: vec![group_record("platform-eng", "Platform Engineering", Permission::Admin)],
            repo_statuses: vec![],
        };

        let diff = diff_snapshots(&a, &b);
        assert_eq!(
            diff.records,
            vec![RecordDiff::LevelChange {
                repo_project: "TEAM".to_string(),
                repo: "repo-a".to_string(),
                principal: Principal {
                    id: "platform-eng".to_string(),
                    label: "Platform Engineering".to_string(),
                },
                access_type: AccessType::Group,
                scope: GrantScope::Repo,
                from: Permission::Read,
                to: Permission::Admin,
                kind: LevelChangeKind::Escalation,
            }]
        );
    }

    #[test]
    fn demotion_when_group_own_grant_level_falls() {
        let a = Snapshot {
            records: vec![group_record("platform-eng", "Platform Engineering", Permission::Admin)],
            repo_statuses: vec![],
        };
        let b = Snapshot {
            records: vec![group_record("platform-eng", "Platform Engineering", Permission::Write)],
            repo_statuses: vec![],
        };

        let diff = diff_snapshots(&a, &b);
        assert_eq!(
            diff.records,
            vec![RecordDiff::LevelChange {
                repo_project: "TEAM".to_string(),
                repo: "repo-a".to_string(),
                principal: Principal {
                    id: "platform-eng".to_string(),
                    label: "Platform Engineering".to_string(),
                },
                access_type: AccessType::Group,
                scope: GrantScope::Repo,
                from: Permission::Admin,
                to: Permission::Write,
                kind: LevelChangeKind::Demotion,
            }]
        );
    }

    #[test]
    fn direct_and_group_grants_sharing_an_id_are_diffed_independently() {
        // Proves access_type is part of the diff key: a Direct grant disappearing must not
        // be masked by an unrelated Group grant that shares the same principal id, and vice versa.
        let a = Snapshot {
            records: vec![
                record("shared-id", "Ada (user)", Permission::Read),
                group_record("shared-id", "Shared Slug (group)", Permission::Write),
            ],
            repo_statuses: vec![],
        };
        let b = Snapshot {
            records: vec![group_record("shared-id", "Shared Slug (group)", Permission::Write)],
            repo_statuses: vec![],
        };

        let diff = diff_snapshots(&a, &b);
        assert_eq!(
            diff.records,
            vec![RecordDiff::Revoke(record("shared-id", "Ada (user)", Permission::Read))]
        );
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

    #[test]
    fn grant_for_member_record_when_record_only_in_b() {
        let a = Snapshot::default();
        let b = Snapshot {
            records: vec![member_record("platform-eng", "acct-1", "Ada", Permission::Write)],
            repo_statuses: vec![],
        };

        let diff = diff_snapshots(&a, &b);
        assert_eq!(
            diff.records,
            vec![RecordDiff::Grant(member_record(
                "platform-eng",
                "acct-1",
                "Ada",
                Permission::Write
            ))]
        );
    }

    #[test]
    fn revoke_for_member_record_when_record_only_in_a() {
        let a = Snapshot {
            records: vec![member_record("platform-eng", "acct-1", "Ada", Permission::Write)],
            repo_statuses: vec![],
        };
        let b = Snapshot::default();

        let diff = diff_snapshots(&a, &b);
        assert_eq!(
            diff.records,
            vec![RecordDiff::Revoke(member_record(
                "platform-eng",
                "acct-1",
                "Ada",
                Permission::Write
            ))]
        );
    }

    #[test]
    fn escalation_when_member_record_level_rises() {
        let a = Snapshot {
            records: vec![member_record("platform-eng", "acct-1", "Ada", Permission::Read)],
            repo_statuses: vec![],
        };
        let b = Snapshot {
            records: vec![member_record("platform-eng", "acct-1", "Ada", Permission::Admin)],
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
                access_type: AccessType::Member("platform-eng".to_string()),
                scope: GrantScope::Repo,
                from: Permission::Read,
                to: Permission::Admin,
                kind: LevelChangeKind::Escalation,
            }]
        );
    }

    #[test]
    fn demotion_when_member_record_level_falls() {
        let a = Snapshot {
            records: vec![member_record("platform-eng", "acct-1", "Ada", Permission::Admin)],
            repo_statuses: vec![],
        };
        let b = Snapshot {
            records: vec![member_record("platform-eng", "acct-1", "Ada", Permission::Write)],
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
                access_type: AccessType::Member("platform-eng".to_string()),
                scope: GrantScope::Repo,
                from: Permission::Admin,
                to: Permission::Write,
                kind: LevelChangeKind::Demotion,
            }]
        );
    }

    #[test]
    fn new_member_grant_alongside_unchanged_direct_grant_is_a_grant_not_an_escalation() {
        // ADR-0001: records are diffed per grant-source, never collapsed into an "effective
        // permission" per Principal. A user picking up a higher-level Member grant via a new
        // group membership, while their unrelated Direct grant is untouched, must surface as
        // an independent Grant on the Member record — not an Escalation of the Direct one.
        let a = Snapshot {
            records: vec![record("acct-1", "Ada", Permission::Read)],
            repo_statuses: vec![],
        };
        let b = Snapshot {
            records: vec![
                record("acct-1", "Ada", Permission::Read),
                member_record("platform-eng", "acct-1", "Ada", Permission::Admin),
            ],
            repo_statuses: vec![],
        };

        let diff = diff_snapshots(&a, &b);
        assert_eq!(
            diff.records,
            vec![RecordDiff::Grant(member_record(
                "platform-eng",
                "acct-1",
                "Ada",
                Permission::Admin
            ))]
        );
    }

    #[test]
    fn direct_and_member_grants_sharing_a_principal_are_diffed_independently() {
        // A principal can hold both a Direct grant and a Member grant on the same repo at
        // different levels; they must never be collapsed into one record (ADR-0001) — a
        // Direct revoke must not be masked by an untouched Member grant, and vice versa.
        let a = Snapshot {
            records: vec![
                record("acct-1", "Ada", Permission::Write),
                member_record("platform-eng", "acct-1", "Ada", Permission::Read),
            ],
            repo_statuses: vec![],
        };
        let b = Snapshot {
            records: vec![member_record("platform-eng", "acct-1", "Ada", Permission::Read)],
            repo_statuses: vec![],
        };

        let diff = diff_snapshots(&a, &b);
        assert_eq!(
            diff.records,
            vec![RecordDiff::Revoke(record("acct-1", "Ada", Permission::Write))]
        );
    }

    #[test]
    fn repo_and_project_grants_of_same_access_type_and_level_are_diffed_independently() {
        // ADR-0012: scope is part of the diff key. A Repo-level and a Project-level Direct
        // grant for the same principal+repo at the same level must surface as two independent
        // Grant entries, never collapsed into one because their (access_type, permission) match.
        let a = Snapshot::default();
        let b = Snapshot {
            records: vec![
                record("acct-1", "Ada", Permission::Admin),
                project_record("acct-1", "Ada", Permission::Admin),
            ],
            repo_statuses: vec![],
        };

        let diff = diff_snapshots(&a, &b);
        assert_eq!(
            diff.records,
            vec![
                RecordDiff::Grant(record("acct-1", "Ada", Permission::Admin)),
                RecordDiff::Grant(project_record("acct-1", "Ada", Permission::Admin)),
            ]
        );
    }

    #[test]
    fn repo_and_project_escalations_of_the_same_grant_source_are_distinguishable_by_scope() {
        // RecordDiff::LevelChange must carry `scope` itself (not just Grant/Revoke, which carry
        // the whole PermissionRecord) — otherwise two independent escalations that differ only by
        // scope would produce two structurally-identical LevelChange values, indistinguishable to
        // any consumer despite coming from different grant sources.
        let a = Snapshot {
            records: vec![
                record("acct-1", "Ada", Permission::Read),
                project_record("acct-1", "Ada", Permission::Read),
            ],
            repo_statuses: vec![],
        };
        let b = Snapshot {
            records: vec![
                record("acct-1", "Ada", Permission::Admin),
                project_record("acct-1", "Ada", Permission::Admin),
            ],
            repo_statuses: vec![],
        };

        let diff = diff_snapshots(&a, &b);
        let scopes: Vec<GrantScope> = diff
            .records
            .iter()
            .map(|d| match d {
                RecordDiff::LevelChange { scope, .. } => *scope,
                other => panic!("expected LevelChange, got {other:?}"),
            })
            .collect();
        assert_eq!(scopes, vec![GrantScope::Repo, GrantScope::Project]);
    }

    #[test]
    fn grant_moving_from_repo_scope_to_project_scope_diffs_as_revoke_plus_grant() {
        // Same principal, repo, access_type, and level — only scope changes between the two
        // Snapshots. Per ADR-0012 this is not a no-op: it reads as a Revoke of the Repo-level
        // record plus a Grant of the Project-level one.
        let a = Snapshot {
            records: vec![record("acct-1", "Ada", Permission::Write)],
            repo_statuses: vec![],
        };
        let b = Snapshot {
            records: vec![project_record("acct-1", "Ada", Permission::Write)],
            repo_statuses: vec![],
        };

        let diff = diff_snapshots(&a, &b);
        assert_eq!(
            diff.records,
            vec![
                RecordDiff::Revoke(record("acct-1", "Ada", Permission::Write)),
                RecordDiff::Grant(project_record("acct-1", "Ada", Permission::Write)),
            ]
        );
    }
}
