use crate::model::{AccessType, Permission, PermissionRecord, Principal, RepoFetchStatus, RepoStatus};

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
                .map(|p| PermissionRecord {
                    repo_project: repo_project.to_string(),
                    repo: repo.to_string(),
                    principal: Principal {
                        id: p.account_id,
                        label: p.display_name,
                    },
                    access_type: AccessType::Direct,
                    permission: Permission::parse(&p.permission)
                        .unwrap_or_else(|| panic!("invalid permission string: {}", p.permission)),
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

#[derive(Debug, Clone)]
pub struct RawGroupPermission {
    pub group_slug: String,
    pub group_name: String,
    pub permission: String,
}

/// A repo's already-fetched `permissions-config/groups` response, or a sentinel for a
/// failed fetch — mirrors `RawUsersResponse`'s Ok/FetchFailed split.
#[derive(Debug, Clone)]
pub enum RawGroupsResponse {
    Ok(Vec<RawGroupPermission>),
    FetchFailed,
}

/// Normalizes one repo's raw group-grant response into `PermissionRecord`s with
/// `access_type: Group`, keyed on the group's stable slug (never its display name) per
/// ADR-0002. A group's own grant is recorded whenever this call succeeds, independent of
/// whether its membership is resolvable — membership expansion is a separate concern.
pub fn normalize_repo_group_permissions(
    repo_project: &str,
    repo: &str,
    groups: RawGroupsResponse,
) -> Vec<PermissionRecord> {
    match groups {
        RawGroupsResponse::FetchFailed => Vec::new(),
        RawGroupsResponse::Ok(perms) => perms
            .into_iter()
            .map(|p| PermissionRecord {
                repo_project: repo_project.to_string(),
                repo: repo.to_string(),
                principal: Principal {
                    id: p.group_slug,
                    label: p.group_name,
                },
                access_type: AccessType::Group,
                permission: Permission::parse(&p.permission)
                    .unwrap_or_else(|| panic!("invalid permission string: {}", p.permission)),
            })
            .collect(),
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
        let records = normalize_repo_group_permissions(
            "TEAM",
            "repo-a",
            RawGroupsResponse::Ok(vec![RawGroupPermission {
                group_slug: "platform-eng".to_string(),
                group_name: "Platform Engineering".to_string(),
                permission: "write".to_string(),
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
                permission: Permission::Write,
            }]
        );
    }

    #[test]
    fn group_grant_recorded_regardless_of_member_resolvability() {
        // Membership expansion is out of scope here (PD-4); the group's own grant must
        // still be recorded as long as the groups-permissions-config call itself succeeded.
        let records = normalize_repo_group_permissions(
            "TEAM",
            "repo-a",
            RawGroupsResponse::Ok(vec![RawGroupPermission {
                group_slug: "empty-group".to_string(),
                group_name: "Empty Group".to_string(),
                permission: "admin".to_string(),
            }]),
        );

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].access_type, AccessType::Group);
    }

    #[test]
    fn group_fetch_failure_produces_zero_records() {
        let records =
            normalize_repo_group_permissions("TEAM", "repo-a", RawGroupsResponse::FetchFailed);

        assert!(records.is_empty());
    }
}
