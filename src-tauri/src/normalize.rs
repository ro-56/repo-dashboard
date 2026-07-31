use crate::model::{
    AccessType, GrantScope, GroupMembershipStatus, Permission, PermissionRecord, Principal,
    ProjectFetchStatus, RepoFetchStatus, RepoStatus,
};

#[derive(Debug, Clone)]
pub struct RawUserPermission {
    pub account_id: String,
    pub display_name: String,
    pub permission: String,
}

/// A repo's already-fetched `permissions-config/users` response, or a sentinel for a
/// failed fetch (e.g. 404/network error) — the two must never be conflated.
#[derive(Debug, Clone)]
pub enum RawUsersResponse {
    Ok(Vec<RawUserPermission>),
    FetchFailed,
}

/// Normalizes one repo's raw Direct-grant response into `PermissionRecord`s plus the
/// repo's fetch status. A `FetchFailed` response yields zero records — never "zero grants".
pub fn normalize_repo_permissions(
    repo_project: &str,
    repo: &str,
    users: RawUsersResponse,
) -> (Vec<PermissionRecord>, RepoFetchStatus) {
    match users {
        RawUsersResponse::FetchFailed => (
            Vec::new(),
            RepoFetchStatus {
                repo_project: repo_project.to_string(),
                repo: repo.to_string(),
                status: RepoStatus::FetchFailed,
            },
        ),
        RawUsersResponse::Ok(perms) => {
            let records = perms
                .into_iter()
                .filter_map(|p| {
                    let permission = match Permission::parse(&p.permission) {
                        Some(permission) => permission,
                        None => {
                            eprintln!(
                                "warning: skipping record with unrecognized permission {:?} \
                                 (repo_project={repo_project}, repo={repo}, principal={})",
                                p.permission, p.account_id
                            );
                            return None;
                        }
                    };
                    Some(PermissionRecord {
                        repo_project: repo_project.to_string(),
                        repo: repo.to_string(),
                        principal: Principal {
                            id: p.account_id,
                            label: p.display_name,
                        },
                        access_type: AccessType::Direct,
                        scope: GrantScope::Repo,
                        permission,
                    })
                })
                .collect();
            (
                records,
                RepoFetchStatus {
                    repo_project: repo_project.to_string(),
                    repo: repo.to_string(),
                    status: RepoStatus::Ok,
                },
            )
        }
    }
}

/// Normalizes one Project's raw Direct-grant response (same shape as a repo's) into
/// `PermissionRecord`s tagged `scope: Project` plus the Project's fetch status — mirrors
/// `normalize_repo_permissions` exactly, save for that tag and `ProjectFetchStatus` in place of
/// `RepoFetchStatus`. Callers attribute the result to one repo the Project owns; fanning a
/// Project's grants out across every repo it owns is an orchestration concern, not this
/// function's.
pub fn normalize_project_permissions(
    repo_project: &str,
    repo: &str,
    users: RawUsersResponse,
) -> (Vec<PermissionRecord>, ProjectFetchStatus) {
    match users {
        RawUsersResponse::FetchFailed => (
            Vec::new(),
            ProjectFetchStatus {
                project_key: repo_project.to_string(),
                status: RepoStatus::FetchFailed,
            },
        ),
        RawUsersResponse::Ok(perms) => {
            let records = perms
                .into_iter()
                .filter_map(|p| {
                    let permission = match Permission::parse(&p.permission) {
                        Some(permission) => permission,
                        None => {
                            eprintln!(
                                "warning: skipping record with unrecognized permission {:?} \
                                 (repo_project={repo_project}, repo={repo}, principal={})",
                                p.permission, p.account_id
                            );
                            return None;
                        }
                    };
                    Some(PermissionRecord {
                        repo_project: repo_project.to_string(),
                        repo: repo.to_string(),
                        principal: Principal {
                            id: p.account_id,
                            label: p.display_name,
                        },
                        access_type: AccessType::Direct,
                        scope: GrantScope::Project,
                        permission,
                    })
                })
                .collect();
            (
                records,
                ProjectFetchStatus {
                    project_key: repo_project.to_string(),
                    status: RepoStatus::Ok,
                },
            )
        }
    }
}

#[derive(Debug, Clone)]
pub struct RawMember {
    pub account_id: String,
    pub display_name: String,
}

/// A group's already-fetched membership-list response, or a sentinel for a failed/inaccessible
/// fetch. Distinct from `Ok(vec![])`, which means "fetched successfully, confirmed empty".
#[derive(Debug, Clone)]
pub enum RawGroupMembersResponse {
    Ok(Vec<RawMember>),
    FetchFailed,
}

#[derive(Debug, Clone)]
pub struct RawGroupPermission {
    pub group_slug: String,
    pub group_name: String,
    pub permission: String,
    pub members: RawGroupMembersResponse,
}

/// A repo's already-fetched `permissions-config/groups` response, or a sentinel for a
/// failed fetch — mirrors `RawUsersResponse`'s Ok/FetchFailed split.
#[derive(Debug, Clone)]
pub enum RawGroupsResponse {
    Ok(Vec<RawGroupPermission>),
    FetchFailed,
}

/// Normalizes one repo's raw group-grant response into `PermissionRecord`s. Each group
/// produces its own `access_type: Group` record, keyed on its stable slug (never its display
/// name) per ADR-0002, whenever the groups call itself succeeds — independent of whether its
/// membership is resolvable. When membership *is* resolvable, each member additionally
/// produces its own `access_type: Member(group_id)` record at the group's permission level,
/// layered on top of (never replacing) the group's own record. Returns the group records plus
/// one `GroupMembershipStatus` per group recording whether its membership was resolved.
pub fn normalize_repo_group_permissions(
    repo_project: &str,
    repo: &str,
    groups: RawGroupsResponse,
) -> (Vec<PermissionRecord>, Vec<GroupMembershipStatus>) {
    match groups {
        RawGroupsResponse::FetchFailed => (Vec::new(), Vec::new()),
        RawGroupsResponse::Ok(perms) => {
            let mut records = Vec::new();
            let mut membership_statuses = Vec::new();

            for p in perms {
                let permission = match Permission::parse(&p.permission) {
                    Some(permission) => permission,
                    None => {
                        eprintln!(
                            "warning: skipping group record with unrecognized permission {:?} \
                             (repo_project={repo_project}, repo={repo}, principal={})",
                            p.permission, p.group_slug
                        );
                        continue;
                    }
                };

                records.push(PermissionRecord {
                    repo_project: repo_project.to_string(),
                    repo: repo.to_string(),
                    principal: Principal {
                        id: p.group_slug.clone(),
                        label: p.group_name.clone(),
                    },
                    access_type: AccessType::Group,
                    scope: GrantScope::Repo,
                    permission,
                });

                let members_resolved = match p.members {
                    RawGroupMembersResponse::FetchFailed => false,
                    RawGroupMembersResponse::Ok(members) => {
                        records.extend(members.into_iter().map(|m| PermissionRecord {
                            repo_project: repo_project.to_string(),
                            repo: repo.to_string(),
                            principal: Principal {
                                id: m.account_id,
                                label: m.display_name,
                            },
                            access_type: AccessType::Member(p.group_slug.clone()),
                            scope: GrantScope::Repo,
                            permission,
                        }));
                        true
                    }
                };

                membership_statuses.push(GroupMembershipStatus {
                    repo_project: repo_project.to_string(),
                    repo: repo.to_string(),
                    group_id: p.group_slug,
                    members_resolved,
                });
            }

            (records, membership_statuses)
        }
    }
}

/// Normalizes one Project's raw group-grant response (same shape as a repo's) into
/// `PermissionRecord`s tagged `scope: Project` — mirrors `normalize_repo_group_permissions`
/// exactly, reusing its group-membership-resolution logic and `GroupMembershipStatus` tracking
/// unmodified. Each group still produces its own `Group` record, and, when membership is
/// resolvable, one `Member(group_id)` record per member — both tagged `scope: Project` here.
pub fn normalize_project_group_permissions(
    repo_project: &str,
    repo: &str,
    groups: RawGroupsResponse,
) -> (Vec<PermissionRecord>, Vec<GroupMembershipStatus>) {
    match groups {
        RawGroupsResponse::FetchFailed => (Vec::new(), Vec::new()),
        RawGroupsResponse::Ok(perms) => {
            let mut records = Vec::new();
            let mut membership_statuses = Vec::new();

            for p in perms {
                let permission = match Permission::parse(&p.permission) {
                    Some(permission) => permission,
                    None => {
                        eprintln!(
                            "warning: skipping group record with unrecognized permission {:?} \
                             (repo_project={repo_project}, repo={repo}, principal={})",
                            p.permission, p.group_slug
                        );
                        continue;
                    }
                };

                records.push(PermissionRecord {
                    repo_project: repo_project.to_string(),
                    repo: repo.to_string(),
                    principal: Principal {
                        id: p.group_slug.clone(),
                        label: p.group_name.clone(),
                    },
                    access_type: AccessType::Group,
                    scope: GrantScope::Project,
                    permission,
                });

                let members_resolved = match p.members {
                    RawGroupMembersResponse::FetchFailed => false,
                    RawGroupMembersResponse::Ok(members) => {
                        records.extend(members.into_iter().map(|m| PermissionRecord {
                            repo_project: repo_project.to_string(),
                            repo: repo.to_string(),
                            principal: Principal {
                                id: m.account_id,
                                label: m.display_name,
                            },
                            access_type: AccessType::Member(p.group_slug.clone()),
                            scope: GrantScope::Project,
                            permission,
                        }));
                        true
                    }
                };

                membership_statuses.push(GroupMembershipStatus {
                    repo_project: repo_project.to_string(),
                    repo: repo.to_string(),
                    group_id: p.group_slug,
                    members_resolved,
                });
            }

            (records, membership_statuses)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_grant_produces_record_and_ok_status() {
        let (records, status) = normalize_repo_permissions(
            "TEAM",
            "repo-a",
            RawUsersResponse::Ok(vec![RawUserPermission {
                account_id: "acct-1".to_string(),
                display_name: "Ada Lovelace".to_string(),
                permission: "admin".to_string(),
            }]),
        );

        assert_eq!(
            records,
            vec![PermissionRecord {
                repo_project: "TEAM".to_string(),
                repo: "repo-a".to_string(),
                principal: Principal {
                    id: "acct-1".to_string(),
                    label: "Ada Lovelace".to_string(),
                },
                access_type: AccessType::Direct,
                scope: GrantScope::Repo,
                permission: Permission::Admin,
            }]
        );
        assert_eq!(
            status,
            RepoFetchStatus {
                repo_project: "TEAM".to_string(),
                repo: "repo-a".to_string(),
                status: RepoStatus::Ok,
            }
        );
    }

    #[test]
    fn empty_direct_grants_produces_zero_records_and_ok_status() {
        let (records, status) =
            normalize_repo_permissions("TEAM", "repo-a", RawUsersResponse::Ok(vec![]));

        assert!(records.is_empty());
        assert_eq!(status.status, RepoStatus::Ok);
    }

    #[test]
    fn fetch_failure_produces_zero_records_and_fetch_failed_status() {
        let (records, status) =
            normalize_repo_permissions("TEAM", "repo-a", RawUsersResponse::FetchFailed);

        assert!(records.is_empty());
        assert_eq!(
            status,
            RepoFetchStatus {
                repo_project: "TEAM".to_string(),
                repo: "repo-a".to_string(),
                status: RepoStatus::FetchFailed,
            }
        );
    }

    #[test]
    fn create_repo_permission_produces_a_create_repo_record() {
        let (records, status) = normalize_repo_permissions(
            "TEAM",
            "repo-a",
            RawUsersResponse::Ok(vec![RawUserPermission {
                account_id: "acct-1".to_string(),
                display_name: "Ada Lovelace".to_string(),
                permission: "create-repo".to_string(),
            }]),
        );

        assert_eq!(records[0].permission, Permission::CreateRepo);
        assert_eq!(status.status, RepoStatus::Ok);
    }

    #[test]
    fn unrecognized_permission_is_skipped_and_repo_status_stays_ok() {
        let (records, status) = normalize_repo_permissions(
            "TEAM",
            "repo-a",
            RawUsersResponse::Ok(vec![
                RawUserPermission {
                    account_id: "acct-1".to_string(),
                    display_name: "Ada Lovelace".to_string(),
                    permission: "superadmin".to_string(),
                },
                RawUserPermission {
                    account_id: "acct-2".to_string(),
                    display_name: "Grace Hopper".to_string(),
                    permission: "read".to_string(),
                },
            ]),
        );

        // The unrecognized record is dropped; the valid one alongside it still lands.
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].principal.id, "acct-2");
        assert_eq!(status.status, RepoStatus::Ok);
    }

    #[test]
    fn permission_levels_parse_case_insensitively() {
        let (records, _) = normalize_repo_permissions(
            "TEAM",
            "repo-a",
            RawUsersResponse::Ok(vec![
                RawUserPermission {
                    account_id: "acct-1".to_string(),
                    display_name: "Reader".to_string(),
                    permission: "READ".to_string(),
                },
                RawUserPermission {
                    account_id: "acct-2".to_string(),
                    display_name: "Writer".to_string(),
                    permission: "Write".to_string(),
                },
            ]),
        );

        assert_eq!(records[0].permission, Permission::Read);
        assert_eq!(records[1].permission, Permission::Write);
    }

    #[test]
    fn group_grant_produces_record_keyed_on_slug_not_display_name() {
        let (records, _) = normalize_repo_group_permissions(
            "TEAM",
            "repo-a",
            RawGroupsResponse::Ok(vec![RawGroupPermission {
                group_slug: "platform-eng".to_string(),
                group_name: "Platform Engineering".to_string(),
                permission: "write".to_string(),
                members: RawGroupMembersResponse::FetchFailed,
            }]),
        );

        assert_eq!(
            records,
            vec![PermissionRecord {
                repo_project: "TEAM".to_string(),
                repo: "repo-a".to_string(),
                principal: Principal {
                    id: "platform-eng".to_string(),
                    label: "Platform Engineering".to_string(),
                },
                access_type: AccessType::Group,
                scope: GrantScope::Repo,
                permission: Permission::Write,
            }]
        );
    }

    #[test]
    fn group_create_repo_permission_produces_a_create_repo_record() {
        let (records, _) = normalize_repo_group_permissions(
            "TEAM",
            "repo-a",
            RawGroupsResponse::Ok(vec![RawGroupPermission {
                group_slug: "platform-eng".to_string(),
                group_name: "Platform Engineering".to_string(),
                permission: "create-repo".to_string(),
                members: RawGroupMembersResponse::FetchFailed,
            }]),
        );

        assert_eq!(records[0].permission, Permission::CreateRepo);
    }

    #[test]
    fn group_grant_with_unrecognized_permission_is_skipped_entirely() {
        let (records, statuses) = normalize_repo_group_permissions(
            "TEAM",
            "repo-a",
            RawGroupsResponse::Ok(vec![RawGroupPermission {
                group_slug: "platform-eng".to_string(),
                group_name: "Platform Engineering".to_string(),
                permission: "superadmin".to_string(),
                members: RawGroupMembersResponse::Ok(vec![RawMember {
                    account_id: "acct-1".to_string(),
                    display_name: "Ada Lovelace".to_string(),
                }]),
            }]),
        );

        // Neither the group's own record nor its member records survive an unrecognized
        // permission string — both derive from the same invalid raw value.
        assert!(records.is_empty());
        assert!(statuses.is_empty());
    }

    #[test]
    fn group_grant_recorded_regardless_of_member_resolvability() {
        let (records, _) = normalize_repo_group_permissions(
            "TEAM",
            "repo-a",
            RawGroupsResponse::Ok(vec![RawGroupPermission {
                group_slug: "empty-group".to_string(),
                group_name: "Empty Group".to_string(),
                permission: "admin".to_string(),
                members: RawGroupMembersResponse::FetchFailed,
            }]),
        );

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].access_type, AccessType::Group);
    }

    #[test]
    fn group_fetch_failure_produces_zero_records() {
        let (records, statuses) =
            normalize_repo_group_permissions("TEAM", "repo-a", RawGroupsResponse::FetchFailed);

        assert!(records.is_empty());
        assert!(statuses.is_empty());
    }

    #[test]
    fn resolvable_nonempty_membership_produces_one_member_record_per_member_at_group_level() {
        let (records, statuses) = normalize_repo_group_permissions(
            "TEAM",
            "repo-a",
            RawGroupsResponse::Ok(vec![RawGroupPermission {
                group_slug: "platform-eng".to_string(),
                group_name: "Platform Engineering".to_string(),
                permission: "write".to_string(),
                members: RawGroupMembersResponse::Ok(vec![
                    RawMember {
                        account_id: "acct-1".to_string(),
                        display_name: "Ada Lovelace".to_string(),
                    },
                    RawMember {
                        account_id: "acct-2".to_string(),
                        display_name: "Grace Hopper".to_string(),
                    },
                ]),
            }]),
        );

        assert_eq!(
            records,
            vec![
                PermissionRecord {
                    repo_project: "TEAM".to_string(),
                    repo: "repo-a".to_string(),
                    principal: Principal {
                        id: "platform-eng".to_string(),
                        label: "Platform Engineering".to_string(),
                    },
                    access_type: AccessType::Group,
                    scope: GrantScope::Repo,
                    permission: Permission::Write,
                },
                PermissionRecord {
                    repo_project: "TEAM".to_string(),
                    repo: "repo-a".to_string(),
                    principal: Principal {
                        id: "acct-1".to_string(),
                        label: "Ada Lovelace".to_string(),
                    },
                    access_type: AccessType::Member("platform-eng".to_string()),
                    scope: GrantScope::Repo,
                    permission: Permission::Write,
                },
                PermissionRecord {
                    repo_project: "TEAM".to_string(),
                    repo: "repo-a".to_string(),
                    principal: Principal {
                        id: "acct-2".to_string(),
                        label: "Grace Hopper".to_string(),
                    },
                    access_type: AccessType::Member("platform-eng".to_string()),
                    scope: GrantScope::Repo,
                    permission: Permission::Write,
                },
            ]
        );
        assert_eq!(
            statuses,
            vec![GroupMembershipStatus {
                repo_project: "TEAM".to_string(),
                repo: "repo-a".to_string(),
                group_id: "platform-eng".to_string(),
                members_resolved: true,
            }]
        );
    }

    #[test]
    fn resolvable_empty_membership_produces_zero_member_records_and_resolved_true() {
        let (records, statuses) = normalize_repo_group_permissions(
            "TEAM",
            "repo-a",
            RawGroupsResponse::Ok(vec![RawGroupPermission {
                group_slug: "empty-group".to_string(),
                group_name: "Empty Group".to_string(),
                permission: "admin".to_string(),
                members: RawGroupMembersResponse::Ok(vec![]),
            }]),
        );

        // Only the group's own record — no Member records for a confirmed-empty group.
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].access_type, AccessType::Group);
        assert_eq!(
            statuses,
            vec![GroupMembershipStatus {
                repo_project: "TEAM".to_string(),
                repo: "repo-a".to_string(),
                group_id: "empty-group".to_string(),
                members_resolved: true,
            }]
        );
    }

    #[test]
    fn unresolvable_membership_produces_zero_member_records_and_resolved_false() {
        let (records, statuses) = normalize_repo_group_permissions(
            "TEAM",
            "repo-a",
            RawGroupsResponse::Ok(vec![RawGroupPermission {
                group_slug: "locked-group".to_string(),
                group_name: "Locked Group".to_string(),
                permission: "read".to_string(),
                members: RawGroupMembersResponse::FetchFailed,
            }]),
        );

        // Only the group's own record — never conflated with a confirmed-empty group.
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].access_type, AccessType::Group);
        assert_eq!(
            statuses,
            vec![GroupMembershipStatus {
                repo_project: "TEAM".to_string(),
                repo: "repo-a".to_string(),
                group_id: "locked-group".to_string(),
                members_resolved: false,
            }]
        );
    }

    #[test]
    fn project_direct_grant_produces_project_scoped_record_and_ok_status() {
        let (records, status) = normalize_project_permissions(
            "TEAM",
            "repo-a",
            RawUsersResponse::Ok(vec![RawUserPermission {
                account_id: "acct-1".to_string(),
                display_name: "Ada Lovelace".to_string(),
                permission: "admin".to_string(),
            }]),
        );

        assert_eq!(
            records,
            vec![PermissionRecord {
                repo_project: "TEAM".to_string(),
                repo: "repo-a".to_string(),
                principal: Principal {
                    id: "acct-1".to_string(),
                    label: "Ada Lovelace".to_string(),
                },
                access_type: AccessType::Direct,
                scope: GrantScope::Project,
                permission: Permission::Admin,
            }]
        );
        assert_eq!(
            status,
            ProjectFetchStatus {
                project_key: "TEAM".to_string(),
                status: RepoStatus::Ok,
            }
        );
    }

    #[test]
    fn project_create_repo_permission_produces_a_create_repo_record() {
        let (records, status) = normalize_project_permissions(
            "TEAM",
            "repo-a",
            RawUsersResponse::Ok(vec![RawUserPermission {
                account_id: "acct-1".to_string(),
                display_name: "Ada Lovelace".to_string(),
                permission: "create-repo".to_string(),
            }]),
        );

        assert_eq!(records[0].permission, Permission::CreateRepo);
        assert_eq!(status.status, RepoStatus::Ok);
    }

    #[test]
    fn project_direct_fetch_failure_produces_zero_records_and_fetch_failed_status() {
        let (records, status) =
            normalize_project_permissions("TEAM", "repo-a", RawUsersResponse::FetchFailed);

        assert!(records.is_empty());
        assert_eq!(
            status,
            ProjectFetchStatus {
                project_key: "TEAM".to_string(),
                status: RepoStatus::FetchFailed,
            }
        );
    }

    #[test]
    fn project_group_create_repo_permission_produces_a_create_repo_record() {
        let (records, _) = normalize_project_group_permissions(
            "TEAM",
            "repo-a",
            RawGroupsResponse::Ok(vec![RawGroupPermission {
                group_slug: "platform-eng".to_string(),
                group_name: "Platform Engineering".to_string(),
                permission: "create-repo".to_string(),
                members: RawGroupMembersResponse::FetchFailed,
            }]),
        );

        assert_eq!(records[0].permission, Permission::CreateRepo);
    }

    #[test]
    fn project_group_grant_produces_group_and_member_records_scoped_to_project() {
        let (records, statuses) = normalize_project_group_permissions(
            "TEAM",
            "repo-a",
            RawGroupsResponse::Ok(vec![RawGroupPermission {
                group_slug: "platform-eng".to_string(),
                group_name: "Platform Engineering".to_string(),
                permission: "write".to_string(),
                members: RawGroupMembersResponse::Ok(vec![RawMember {
                    account_id: "acct-1".to_string(),
                    display_name: "Ada Lovelace".to_string(),
                }]),
            }]),
        );

        assert_eq!(
            records,
            vec![
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
                        id: "acct-1".to_string(),
                        label: "Ada Lovelace".to_string(),
                    },
                    access_type: AccessType::Member("platform-eng".to_string()),
                    scope: GrantScope::Project,
                    permission: Permission::Write,
                },
            ]
        );
        assert_eq!(
            statuses,
            vec![GroupMembershipStatus {
                repo_project: "TEAM".to_string(),
                repo: "repo-a".to_string(),
                group_id: "platform-eng".to_string(),
                members_resolved: true,
            }]
        );
    }

    #[test]
    fn project_group_fetch_failure_produces_zero_records_and_zero_statuses() {
        let (records, statuses) =
            normalize_project_group_permissions("TEAM", "repo-a", RawGroupsResponse::FetchFailed);

        assert!(records.is_empty());
        assert!(statuses.is_empty());
    }

    #[test]
    fn project_group_with_unresolvable_membership_produces_zero_member_records_and_resolved_false() {
        let (records, statuses) = normalize_project_group_permissions(
            "TEAM",
            "repo-a",
            RawGroupsResponse::Ok(vec![RawGroupPermission {
                group_slug: "locked-group".to_string(),
                group_name: "Locked Group".to_string(),
                permission: "read".to_string(),
                members: RawGroupMembersResponse::FetchFailed,
            }]),
        );

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].access_type, AccessType::Group);
        assert_eq!(records[0].scope, GrantScope::Project);
        assert_eq!(
            statuses,
            vec![GroupMembershipStatus {
                repo_project: "TEAM".to_string(),
                repo: "repo-a".to_string(),
                group_id: "locked-group".to_string(),
                members_resolved: false,
            }]
        );
    }
}
