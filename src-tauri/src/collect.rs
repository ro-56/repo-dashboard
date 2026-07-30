//! Orchestrates one Run: discovers repos, fetches each repo's Direct and Group grants,
//! resolves each encountered group's membership at most once per Run (cached by group id
//! and reused across every repo that group grants access to), normalizes everything via the
//! existing PD-2 `normalize_repo_permissions` and PD-3/PD-4 `normalize_repo_group_permissions`
//! unmodified, and persists the result as one new immutable Snapshot. The `run_now` Tauri
//! command is a thin wrapper around `collect_and_store` — all meaningful logic lives here,
//! covered by `cargo test`.

use std::collections::HashMap;

use rusqlite::Connection;

use crate::client::{BitbucketClient, ClientError};
use crate::diff::Snapshot;
use crate::model::{
    GroupMembershipStatus, PermissionRecord, ProjectFetchStatus, RepoFetchStatus, RepoStatus,
};
use crate::normalize::{
    normalize_project_group_permissions, normalize_project_permissions,
    normalize_repo_group_permissions, normalize_repo_permissions, RawGroupMembersResponse,
    RawGroupPermission, RawGroupsResponse, RawMember, RawUsersResponse,
};
use crate::storage::save_snapshot;

/// A Discovery failure (glossary, `CONTEXT.md`) aborts before any Snapshot row exists.
/// `CredentialRejected` covers a 401 at any point in the Run — discovery or a later call —
/// since the credential is dead workspace-wide, not scoped to one call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunError {
    CredentialRejected,
    DiscoveryFailed(String),
    StorageFailed(String),
}

/// Runs one full collection pass against `workspace` and persists it as a new Snapshot,
/// returning the new `snapshots.id`. A per-repo Direct- or Group-permissions failure (after
/// the client's own single retry on `RateLimited`) is recorded as a single
/// `RepoFetchStatus::FetchFailed` for that repo and the Run continues; a 401 anywhere aborts
/// the whole Run with no Snapshot written.
pub async fn collect_and_store<C: BitbucketClient>(
    client: &C,
    conn: &mut Connection,
    workspace: &str,
    run_at: &str,
) -> Result<i64, RunError> {
    let repos = client
        .list_repositories(workspace)
        .await
        .map_err(|e| match e {
            ClientError::Unauthorized => RunError::CredentialRejected,
            ClientError::RateLimited => {
                RunError::DiscoveryFailed("rate limited".to_string())
            }
            ClientError::Other(msg) => RunError::DiscoveryFailed(msg),
        })?;

    let mut records: Vec<PermissionRecord> = Vec::new();
    let mut repo_statuses: Vec<RepoFetchStatus> = Vec::new();
    let mut group_membership_statuses: Vec<GroupMembershipStatus> = Vec::new();
    let mut project_statuses: Vec<ProjectFetchStatus> = Vec::new();
    // Caches each group's resolved membership by group id for the lifetime of this Run, so a
    // group granting access to many repos only costs one `list_group_members` call (the
    // caching improvement agreed in the PD-5 design session). Shared between repo-level and
    // Project-level group resolution since both draw from the same group id space.
    let mut member_cache: HashMap<String, Result<Vec<RawMember>, ClientError>> = HashMap::new();
    // Caches each Project's raw Direct/Group responses by project key, so a Project owning
    // many repos only costs one `list_project_direct_permissions` and one
    // `list_project_group_permissions` call (PD-29), mirroring `member_cache` above.
    let mut project_cache: HashMap<String, (RawUsersResponse, RawGroupsResponse)> = HashMap::new();

    for repo in repos {
        let raw = match client.list_direct_permissions(workspace, &repo.repo).await {
            Ok(perms) => RawUsersResponse::Ok(perms),
            Err(ClientError::Unauthorized) => return Err(RunError::CredentialRejected),
            Err(ClientError::RateLimited) | Err(ClientError::Other(_)) => {
                RawUsersResponse::FetchFailed
            }
        };
        let (mut direct_records, direct_status) =
            normalize_repo_permissions(&repo.repo_project, &repo.repo, raw);
        records.append(&mut direct_records);

        let raw_groups = match client.list_group_permissions(workspace, &repo.repo).await {
            Ok(groups) => {
                let mut resolved = Vec::with_capacity(groups.len());
                for group in groups {
                    let members_result = match member_cache.get(&group.group_slug) {
                        Some(cached) => cached.clone(),
                        None => {
                            let result = client
                                .list_group_members(workspace, &group.group_slug)
                                .await;
                            member_cache.insert(group.group_slug.clone(), result.clone());
                            result
                        }
                    };
                    let members = match members_result {
                        Ok(members) => RawGroupMembersResponse::Ok(members),
                        Err(ClientError::Unauthorized) => return Err(RunError::CredentialRejected),
                        Err(ClientError::RateLimited) | Err(ClientError::Other(_)) => {
                            RawGroupMembersResponse::FetchFailed
                        }
                    };
                    resolved.push(RawGroupPermission { members, ..group });
                }
                RawGroupsResponse::Ok(resolved)
            }
            Err(ClientError::Unauthorized) => return Err(RunError::CredentialRejected),
            Err(ClientError::RateLimited) | Err(ClientError::Other(_)) => {
                RawGroupsResponse::FetchFailed
            }
        };
        let group_fetch_failed = matches!(raw_groups, RawGroupsResponse::FetchFailed);
        let (mut group_records, mut membership_statuses) =
            normalize_repo_group_permissions(&repo.repo_project, &repo.repo, raw_groups);
        records.append(&mut group_records);
        group_membership_statuses.append(&mut membership_statuses);

        if !project_cache.contains_key(&repo.repo_project) {
            let raw_project_users = match client
                .list_project_direct_permissions(workspace, &repo.repo_project)
                .await
            {
                Ok(perms) => RawUsersResponse::Ok(perms),
                Err(ClientError::Unauthorized) => return Err(RunError::CredentialRejected),
                Err(ClientError::RateLimited) | Err(ClientError::Other(_)) => {
                    RawUsersResponse::FetchFailed
                }
            };

            let raw_project_groups = match client
                .list_project_group_permissions(workspace, &repo.repo_project)
                .await
            {
                Ok(groups) => {
                    let mut resolved = Vec::with_capacity(groups.len());
                    for group in groups {
                        let members_result = match member_cache.get(&group.group_slug) {
                            Some(cached) => cached.clone(),
                            None => {
                                let result = client
                                    .list_group_members(workspace, &group.group_slug)
                                    .await;
                                member_cache.insert(group.group_slug.clone(), result.clone());
                                result
                            }
                        };
                        let members = match members_result {
                            Ok(members) => RawGroupMembersResponse::Ok(members),
                            Err(ClientError::Unauthorized) => {
                                return Err(RunError::CredentialRejected)
                            }
                            Err(ClientError::RateLimited) | Err(ClientError::Other(_)) => {
                                RawGroupMembersResponse::FetchFailed
                            }
                        };
                        resolved.push(RawGroupPermission { members, ..group });
                    }
                    RawGroupsResponse::Ok(resolved)
                }
                Err(ClientError::Unauthorized) => return Err(RunError::CredentialRejected),
                Err(ClientError::RateLimited) | Err(ClientError::Other(_)) => {
                    RawGroupsResponse::FetchFailed
                }
            };

            let project_status = if matches!(raw_project_users, RawUsersResponse::FetchFailed)
                || matches!(raw_project_groups, RawGroupsResponse::FetchFailed)
            {
                RepoStatus::FetchFailed
            } else {
                RepoStatus::Ok
            };
            project_statuses.push(ProjectFetchStatus {
                project_key: repo.repo_project.clone(),
                status: project_status,
            });

            project_cache
                .insert(repo.repo_project.clone(), (raw_project_users, raw_project_groups));
        }

        let (cached_project_users, cached_project_groups) = project_cache
            .get(&repo.repo_project)
            .expect("just populated above if it was missing");
        let (mut project_direct_records, _) = normalize_project_permissions(
            &repo.repo_project,
            &repo.repo,
            cached_project_users.clone(),
        );
        records.append(&mut project_direct_records);
        let (mut project_group_records, mut project_membership_statuses) =
            normalize_project_group_permissions(
                &repo.repo_project,
                &repo.repo,
                cached_project_groups.clone(),
            );
        records.append(&mut project_group_records);
        group_membership_statuses.append(&mut project_membership_statuses);

        let status = if direct_status.status == RepoStatus::FetchFailed || group_fetch_failed {
            RepoStatus::FetchFailed
        } else {
            RepoStatus::Ok
        };
        repo_statuses.push(RepoFetchStatus {
            repo_project: repo.repo_project,
            repo: repo.repo,
            status,
        });
    }

    let snapshot = Snapshot { records, repo_statuses };
    let snapshot_id = save_snapshot(
        conn,
        run_at,
        &snapshot,
        &group_membership_statuses,
        &project_statuses,
    )
    .map_err(|e| RunError::StorageFailed(e.to_string()))?;
    Ok(snapshot_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::RepoInfo;
    use crate::model::RepoStatus;
    use crate::normalize::{RawGroupMembersResponse, RawGroupPermission, RawUserPermission};
    use crate::storage::{
        init_schema, load_group_membership_statuses, load_project_fetch_statuses, load_snapshot,
    };
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct FakeBitbucketClient {
        repos: Result<Vec<RepoInfo>, ClientError>,
        direct_permissions: HashMap<String, Result<Vec<RawUserPermission>, ClientError>>,
        group_permissions: HashMap<String, Result<Vec<RawGroupPermission>, ClientError>>,
        group_members: HashMap<String, Result<Vec<RawMember>, ClientError>>,
        group_members_call_counts: Mutex<HashMap<String, u32>>,
        project_direct_permissions: HashMap<String, Result<Vec<RawUserPermission>, ClientError>>,
        project_group_permissions: HashMap<String, Result<Vec<RawGroupPermission>, ClientError>>,
        project_direct_permissions_call_counts: Mutex<HashMap<String, u32>>,
        project_group_permissions_call_counts: Mutex<HashMap<String, u32>>,
    }

    impl FakeBitbucketClient {
        fn new(repos: Result<Vec<RepoInfo>, ClientError>) -> Self {
            Self {
                repos,
                direct_permissions: HashMap::new(),
                group_permissions: HashMap::new(),
                group_members: HashMap::new(),
                group_members_call_counts: Mutex::new(HashMap::new()),
                project_direct_permissions: HashMap::new(),
                project_group_permissions: HashMap::new(),
                project_direct_permissions_call_counts: Mutex::new(HashMap::new()),
                project_group_permissions_call_counts: Mutex::new(HashMap::new()),
            }
        }

        fn with_direct_permissions(
            mut self,
            repo: &str,
            result: Result<Vec<RawUserPermission>, ClientError>,
        ) -> Self {
            self.direct_permissions.insert(repo.to_string(), result);
            self
        }

        fn with_group_permissions(
            mut self,
            repo: &str,
            result: Result<Vec<RawGroupPermission>, ClientError>,
        ) -> Self {
            self.group_permissions.insert(repo.to_string(), result);
            self
        }

        fn with_group_members(
            mut self,
            group_slug: &str,
            result: Result<Vec<RawMember>, ClientError>,
        ) -> Self {
            self.group_members.insert(group_slug.to_string(), result);
            self
        }

        fn group_members_call_count(&self, group_slug: &str) -> u32 {
            *self
                .group_members_call_counts
                .lock()
                .unwrap()
                .get(group_slug)
                .unwrap_or(&0)
        }

        fn with_project_direct_permissions(
            mut self,
            project_key: &str,
            result: Result<Vec<RawUserPermission>, ClientError>,
        ) -> Self {
            self.project_direct_permissions.insert(project_key.to_string(), result);
            self
        }

        fn with_project_group_permissions(
            mut self,
            project_key: &str,
            result: Result<Vec<RawGroupPermission>, ClientError>,
        ) -> Self {
            self.project_group_permissions.insert(project_key.to_string(), result);
            self
        }

        fn project_direct_permissions_call_count(&self, project_key: &str) -> u32 {
            *self
                .project_direct_permissions_call_counts
                .lock()
                .unwrap()
                .get(project_key)
                .unwrap_or(&0)
        }

        fn project_group_permissions_call_count(&self, project_key: &str) -> u32 {
            *self
                .project_group_permissions_call_counts
                .lock()
                .unwrap()
                .get(project_key)
                .unwrap_or(&0)
        }
    }

    impl BitbucketClient for FakeBitbucketClient {
        async fn list_repositories(&self, _workspace: &str) -> Result<Vec<RepoInfo>, ClientError> {
            self.repos.clone()
        }

        async fn list_direct_permissions(
            &self,
            _workspace: &str,
            repo: &str,
        ) -> Result<Vec<RawUserPermission>, ClientError> {
            self.direct_permissions
                .get(repo)
                .cloned()
                .unwrap_or(Ok(Vec::new()))
        }

        async fn list_group_permissions(
            &self,
            _workspace: &str,
            repo: &str,
        ) -> Result<Vec<RawGroupPermission>, ClientError> {
            self.group_permissions
                .get(repo)
                .cloned()
                .unwrap_or(Ok(Vec::new()))
        }

        async fn list_group_members(
            &self,
            _workspace: &str,
            group_slug: &str,
        ) -> Result<Vec<RawMember>, ClientError> {
            *self
                .group_members_call_counts
                .lock()
                .unwrap()
                .entry(group_slug.to_string())
                .or_insert(0) += 1;
            self.group_members
                .get(group_slug)
                .cloned()
                .unwrap_or(Ok(Vec::new()))
        }

        async fn list_project_direct_permissions(
            &self,
            _workspace: &str,
            project_key: &str,
        ) -> Result<Vec<RawUserPermission>, ClientError> {
            *self
                .project_direct_permissions_call_counts
                .lock()
                .unwrap()
                .entry(project_key.to_string())
                .or_insert(0) += 1;
            self.project_direct_permissions
                .get(project_key)
                .cloned()
                .unwrap_or(Ok(Vec::new()))
        }

        async fn list_project_group_permissions(
            &self,
            _workspace: &str,
            project_key: &str,
        ) -> Result<Vec<RawGroupPermission>, ClientError> {
            *self
                .project_group_permissions_call_counts
                .lock()
                .unwrap()
                .entry(project_key.to_string())
                .or_insert(0) += 1;
            self.project_group_permissions
                .get(project_key)
                .cloned()
                .unwrap_or(Ok(Vec::new()))
        }
    }

    fn open_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        conn
    }

    fn repo(project: &str, slug: &str) -> RepoInfo {
        RepoInfo { repo_project: project.to_string(), repo: slug.to_string() }
    }

    fn user(id: &str, name: &str, permission: &str) -> RawUserPermission {
        RawUserPermission {
            account_id: id.to_string(),
            display_name: name.to_string(),
            permission: permission.to_string(),
        }
    }

    fn group_perm(slug: &str, name: &str, permission: &str) -> RawGroupPermission {
        RawGroupPermission {
            group_slug: slug.to_string(),
            group_name: name.to_string(),
            permission: permission.to_string(),
            members: RawGroupMembersResponse::FetchFailed,
        }
    }

    fn member(id: &str, name: &str) -> RawMember {
        RawMember { account_id: id.to_string(), display_name: name.to_string() }
    }

    fn snapshot_count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM snapshots", [], |r| r.get(0)).unwrap()
    }

    #[tokio::test]
    async fn happy_path_persists_one_snapshot_with_direct_records_across_repos() {
        let client = FakeBitbucketClient::new(Ok(vec![
            repo("TEAM", "repo-a"),
            repo("TEAM", "repo-b"),
        ]))
        .with_direct_permissions("repo-a", Ok(vec![user("acct-1", "Ada", "admin")]))
        .with_direct_permissions("repo-b", Ok(vec![user("acct-2", "Grace", "read")]));

        let mut conn = open_conn();
        let snapshot_id = collect_and_store(&client, &mut conn, "ws", "2026-01-01T00:00:00Z")
            .await
            .unwrap();

        let loaded = load_snapshot(&conn, snapshot_id).unwrap();
        assert_eq!(loaded.records.len(), 2);
        assert_eq!(loaded.repo_statuses.len(), 2);
        assert!(loaded.repo_statuses.iter().all(|s| s.status == RepoStatus::Ok));
        assert_eq!(snapshot_count(&conn), 1);
    }

    #[tokio::test]
    async fn repo_level_failure_marks_fetch_failed_and_run_continues() {
        let client = FakeBitbucketClient::new(Ok(vec![
            repo("TEAM", "repo-a"),
            repo("TEAM", "repo-b"),
        ]))
        .with_direct_permissions("repo-a", Err(ClientError::Other("boom".to_string())))
        .with_direct_permissions("repo-b", Ok(vec![user("acct-2", "Grace", "read")]));

        let mut conn = open_conn();
        let snapshot_id = collect_and_store(&client, &mut conn, "ws", "2026-01-01T00:00:00Z")
            .await
            .unwrap();

        let loaded = load_snapshot(&conn, snapshot_id).unwrap();
        assert_eq!(loaded.records.len(), 1);
        assert_eq!(
            loaded.repo_statuses.iter().find(|s| s.repo == "repo-a").unwrap().status,
            RepoStatus::FetchFailed
        );
        assert_eq!(
            loaded.repo_statuses.iter().find(|s| s.repo == "repo-b").unwrap().status,
            RepoStatus::Ok
        );
    }

    #[tokio::test]
    async fn group_grant_is_normalized_and_persisted_alongside_direct_grants() {
        let client = FakeBitbucketClient::new(Ok(vec![repo("TEAM", "repo-a")]))
            .with_direct_permissions("repo-a", Ok(vec![user("acct-1", "Ada", "admin")]))
            .with_group_permissions(
                "repo-a",
                Ok(vec![group_perm("platform-eng", "Platform Engineering", "write")]),
            );

        let mut conn = open_conn();
        let snapshot_id = collect_and_store(&client, &mut conn, "ws", "2026-01-01T00:00:00Z")
            .await
            .unwrap();

        let loaded = load_snapshot(&conn, snapshot_id).unwrap();
        assert_eq!(loaded.records.len(), 2);
        assert!(loaded
            .records
            .iter()
            .any(|r| r.access_type == crate::model::AccessType::Direct));
        let group_record = loaded
            .records
            .iter()
            .find(|r| r.access_type == crate::model::AccessType::Group)
            .expect("group record should be persisted");
        assert_eq!(group_record.principal.id, "platform-eng");
        assert_eq!(group_record.permission, crate::model::Permission::Write);
        assert_eq!(loaded.repo_statuses[0].status, RepoStatus::Ok);
    }

    #[tokio::test]
    async fn empty_group_permissions_still_marks_repo_ok() {
        let client = FakeBitbucketClient::new(Ok(vec![repo("TEAM", "repo-a")]))
            .with_group_permissions("repo-a", Ok(vec![]));

        let mut conn = open_conn();
        let snapshot_id = collect_and_store(&client, &mut conn, "ws", "2026-01-01T00:00:00Z")
            .await
            .unwrap();

        let loaded = load_snapshot(&conn, snapshot_id).unwrap();
        assert!(loaded.records.is_empty());
        assert_eq!(loaded.repo_statuses[0].status, RepoStatus::Ok);
    }

    #[tokio::test]
    async fn group_permissions_failure_marks_repo_fetch_failed_even_when_direct_succeeds() {
        let client = FakeBitbucketClient::new(Ok(vec![repo("TEAM", "repo-a")]))
            .with_direct_permissions("repo-a", Ok(vec![user("acct-1", "Ada", "admin")]))
            .with_group_permissions("repo-a", Err(ClientError::Other("boom".to_string())));

        let mut conn = open_conn();
        let snapshot_id = collect_and_store(&client, &mut conn, "ws", "2026-01-01T00:00:00Z")
            .await
            .unwrap();

        let loaded = load_snapshot(&conn, snapshot_id).unwrap();
        // The direct grant that did succeed is still recorded...
        assert_eq!(loaded.records.len(), 1);
        // ...but the repo as a whole is flagged failed, reusing the existing failure path.
        assert_eq!(loaded.repo_statuses[0].status, RepoStatus::FetchFailed);
    }

    #[tokio::test]
    async fn unauthorized_on_discovery_aborts_before_any_snapshot_is_written() {
        let client = FakeBitbucketClient::new(Err(ClientError::Unauthorized));

        let mut conn = open_conn();
        let result = collect_and_store(&client, &mut conn, "ws", "2026-01-01T00:00:00Z").await;

        assert_eq!(result, Err(RunError::CredentialRejected));
        assert_eq!(snapshot_count(&conn), 0);
    }

    #[tokio::test]
    async fn unauthorized_on_a_later_call_aborts_the_whole_run() {
        let client = FakeBitbucketClient::new(Ok(vec![
            repo("TEAM", "repo-a"),
            repo("TEAM", "repo-b"),
        ]))
        .with_direct_permissions("repo-a", Ok(vec![user("acct-1", "Ada", "admin")]))
        .with_direct_permissions("repo-b", Err(ClientError::Unauthorized));

        let mut conn = open_conn();
        let result = collect_and_store(&client, &mut conn, "ws", "2026-01-01T00:00:00Z").await;

        assert_eq!(result, Err(RunError::CredentialRejected));
        assert_eq!(snapshot_count(&conn), 0);
    }

    #[tokio::test]
    async fn unauthorized_on_a_group_permissions_call_aborts_the_whole_run() {
        let client = FakeBitbucketClient::new(Ok(vec![
            repo("TEAM", "repo-a"),
            repo("TEAM", "repo-b"),
        ]))
        .with_direct_permissions("repo-a", Ok(vec![user("acct-1", "Ada", "admin")]))
        .with_group_permissions("repo-b", Err(ClientError::Unauthorized));

        let mut conn = open_conn();
        let result = collect_and_store(&client, &mut conn, "ws", "2026-01-01T00:00:00Z").await;

        assert_eq!(result, Err(RunError::CredentialRejected));
        assert_eq!(snapshot_count(&conn), 0);
    }

    #[tokio::test]
    async fn discovery_failure_other_than_unauthorized_writes_no_snapshot() {
        let client = FakeBitbucketClient::new(Err(ClientError::Other("network error".to_string())));

        let mut conn = open_conn();
        let result = collect_and_store(&client, &mut conn, "ws", "2026-01-01T00:00:00Z").await;

        assert!(matches!(result, Err(RunError::DiscoveryFailed(_))));
        assert_eq!(snapshot_count(&conn), 0);
    }

    #[tokio::test]
    async fn empty_workspace_still_persists_an_empty_snapshot() {
        let client = FakeBitbucketClient::new(Ok(vec![]));

        let mut conn = open_conn();
        let snapshot_id = collect_and_store(&client, &mut conn, "ws", "2026-01-01T00:00:00Z")
            .await
            .unwrap();

        let loaded = load_snapshot(&conn, snapshot_id).unwrap();
        assert!(loaded.records.is_empty());
        assert!(loaded.repo_statuses.is_empty());
    }

    #[tokio::test]
    async fn resolvable_nonempty_group_membership_produces_member_records_at_group_level() {
        let client = FakeBitbucketClient::new(Ok(vec![repo("TEAM", "repo-a")]))
            .with_group_permissions(
                "repo-a",
                Ok(vec![group_perm("platform-eng", "Platform Engineering", "write")]),
            )
            .with_group_members(
                "platform-eng",
                Ok(vec![member("acct-1", "Ada"), member("acct-2", "Grace")]),
            );

        let mut conn = open_conn();
        let snapshot_id = collect_and_store(&client, &mut conn, "ws", "2026-01-01T00:00:00Z")
            .await
            .unwrap();

        let loaded = load_snapshot(&conn, snapshot_id).unwrap();
        // One Group record plus one Member record per resolved member.
        assert_eq!(loaded.records.len(), 3);
        let member_records: Vec<_> = loaded
            .records
            .iter()
            .filter(|r| r.access_type == crate::model::AccessType::Member("platform-eng".to_string()))
            .collect();
        assert_eq!(member_records.len(), 2);
        assert!(member_records
            .iter()
            .all(|r| r.permission == crate::model::Permission::Write));

        let statuses = load_group_membership_statuses(&conn, snapshot_id).unwrap();
        assert_eq!(statuses.len(), 1);
        assert!(statuses[0].members_resolved);
    }

    #[tokio::test]
    async fn resolvable_empty_group_membership_produces_zero_member_records_and_resolved_true() {
        let client = FakeBitbucketClient::new(Ok(vec![repo("TEAM", "repo-a")]))
            .with_group_permissions(
                "repo-a",
                Ok(vec![group_perm("empty-group", "Empty Group", "admin")]),
            )
            .with_group_members("empty-group", Ok(vec![]));

        let mut conn = open_conn();
        let snapshot_id = collect_and_store(&client, &mut conn, "ws", "2026-01-01T00:00:00Z")
            .await
            .unwrap();

        let loaded = load_snapshot(&conn, snapshot_id).unwrap();
        // Only the group's own record — no Member records for a confirmed-empty group.
        assert_eq!(loaded.records.len(), 1);
        assert_eq!(loaded.records[0].access_type, crate::model::AccessType::Group);

        let statuses = load_group_membership_statuses(&conn, snapshot_id).unwrap();
        assert_eq!(statuses.len(), 1);
        assert!(statuses[0].members_resolved);
    }

    #[tokio::test]
    async fn unresolvable_group_membership_produces_zero_member_records_resolved_false_and_run_continues(
    ) {
        let client = FakeBitbucketClient::new(Ok(vec![repo("TEAM", "repo-a")]))
            .with_direct_permissions("repo-a", Ok(vec![user("acct-1", "Ada", "admin")]))
            .with_group_permissions(
                "repo-a",
                Ok(vec![group_perm("locked-group", "Locked Group", "read")]),
            )
            .with_group_members("locked-group", Err(ClientError::Other("boom".to_string())));

        let mut conn = open_conn();
        let snapshot_id = collect_and_store(&client, &mut conn, "ws", "2026-01-01T00:00:00Z")
            .await
            .unwrap();

        let loaded = load_snapshot(&conn, snapshot_id).unwrap();
        // Direct grant + the group's own record — no Member records.
        assert_eq!(loaded.records.len(), 2);
        assert!(loaded
            .records
            .iter()
            .all(|r| !matches!(r.access_type, crate::model::AccessType::Member(_))));
        // The group-permissions call itself succeeded, so the repo is not marked failed.
        assert_eq!(loaded.repo_statuses[0].status, RepoStatus::Ok);

        let statuses = load_group_membership_statuses(&conn, snapshot_id).unwrap();
        assert_eq!(statuses.len(), 1);
        assert!(!statuses[0].members_resolved);
    }

    #[tokio::test]
    async fn a_groups_members_are_fetched_at_most_once_per_run_across_multiple_repos() {
        let client = FakeBitbucketClient::new(Ok(vec![
            repo("TEAM", "repo-a"),
            repo("TEAM", "repo-b"),
        ]))
        .with_group_permissions(
            "repo-a",
            Ok(vec![group_perm("platform-eng", "Platform Engineering", "write")]),
        )
        .with_group_permissions(
            "repo-b",
            Ok(vec![group_perm("platform-eng", "Platform Engineering", "read")]),
        )
        .with_group_members("platform-eng", Ok(vec![member("acct-1", "Ada")]));

        let mut conn = open_conn();
        let snapshot_id = collect_and_store(&client, &mut conn, "ws", "2026-01-01T00:00:00Z")
            .await
            .unwrap();

        assert_eq!(client.group_members_call_count("platform-eng"), 1);

        let loaded = load_snapshot(&conn, snapshot_id).unwrap();
        let member_records: Vec<_> = loaded
            .records
            .iter()
            .filter(|r| matches!(r.access_type, crate::model::AccessType::Member(_)))
            .collect();
        assert_eq!(member_records.len(), 2); // one per repo the group grants access to
    }

    #[tokio::test]
    async fn unauthorized_on_a_group_members_call_aborts_the_whole_run() {
        let client = FakeBitbucketClient::new(Ok(vec![repo("TEAM", "repo-a")]))
            .with_group_permissions(
                "repo-a",
                Ok(vec![group_perm("locked-group", "Locked Group", "read")]),
            )
            .with_group_members("locked-group", Err(ClientError::Unauthorized));

        let mut conn = open_conn();
        let result = collect_and_store(&client, &mut conn, "ws", "2026-01-01T00:00:00Z").await;

        assert_eq!(result, Err(RunError::CredentialRejected));
        assert_eq!(snapshot_count(&conn), 0);
    }

    #[tokio::test]
    async fn run_persists_direct_group_and_member_grants_together_in_one_snapshot() {
        let client = FakeBitbucketClient::new(Ok(vec![repo("TEAM", "repo-a")]))
            .with_direct_permissions("repo-a", Ok(vec![user("acct-1", "Ada", "admin")]))
            .with_group_permissions(
                "repo-a",
                Ok(vec![group_perm("platform-eng", "Platform Engineering", "write")]),
            )
            .with_group_members("platform-eng", Ok(vec![member("acct-2", "Grace")]));

        let mut conn = open_conn();
        let snapshot_id = collect_and_store(&client, &mut conn, "ws", "2026-01-01T00:00:00Z")
            .await
            .unwrap();

        let loaded = load_snapshot(&conn, snapshot_id).unwrap();
        assert_eq!(loaded.records.len(), 3);
        assert!(loaded
            .records
            .iter()
            .any(|r| r.access_type == crate::model::AccessType::Direct));
        assert!(loaded
            .records
            .iter()
            .any(|r| r.access_type == crate::model::AccessType::Group));
        assert!(loaded
            .records
            .iter()
            .any(|r| matches!(r.access_type, crate::model::AccessType::Member(_))));
    }

    #[tokio::test]
    async fn project_direct_grant_is_scoped_to_project_and_persisted_for_its_repo() {
        let client = FakeBitbucketClient::new(Ok(vec![repo("TEAM", "repo-a")]))
            .with_project_direct_permissions("TEAM", Ok(vec![user("acct-9", "Static", "read")]));

        let mut conn = open_conn();
        let snapshot_id = collect_and_store(&client, &mut conn, "ws", "2026-01-01T00:00:00Z")
            .await
            .unwrap();

        let loaded = load_snapshot(&conn, snapshot_id).unwrap();
        let project_record = loaded
            .records
            .iter()
            .find(|r| r.scope == crate::model::GrantScope::Project)
            .expect("project-scoped record should be persisted");
        assert_eq!(project_record.repo, "repo-a");
        assert_eq!(project_record.principal.id, "acct-9");

        let statuses = load_project_fetch_statuses(&conn, snapshot_id).unwrap();
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].project_key, "TEAM");
        assert_eq!(statuses[0].status, RepoStatus::Ok);
    }

    #[tokio::test]
    async fn project_shared_by_multiple_repos_is_fetched_once_and_flattened_to_each_repo() {
        let client = FakeBitbucketClient::new(Ok(vec![
            repo("TEAM", "repo-a"),
            repo("TEAM", "repo-b"),
        ]))
        .with_project_direct_permissions("TEAM", Ok(vec![user("acct-9", "Static", "read")]));

        let mut conn = open_conn();
        let snapshot_id = collect_and_store(&client, &mut conn, "ws", "2026-01-01T00:00:00Z")
            .await
            .unwrap();

        assert_eq!(client.project_direct_permissions_call_count("TEAM"), 1);

        let loaded = load_snapshot(&conn, snapshot_id).unwrap();
        let project_records: Vec<_> = loaded
            .records
            .iter()
            .filter(|r| r.scope == crate::model::GrantScope::Project)
            .collect();
        assert_eq!(project_records.len(), 2);
        assert!(project_records.iter().any(|r| r.repo == "repo-a"));
        assert!(project_records.iter().any(|r| r.repo == "repo-b"));

        let statuses = load_project_fetch_statuses(&conn, snapshot_id).unwrap();
        assert_eq!(statuses.len(), 1);
    }

    #[tokio::test]
    async fn project_group_members_are_resolved_through_the_shared_group_member_cache() {
        let client = FakeBitbucketClient::new(Ok(vec![repo("TEAM", "repo-a")]))
            .with_group_permissions(
                "repo-a",
                Ok(vec![group_perm("platform-eng", "Platform Engineering", "write")]),
            )
            .with_project_group_permissions(
                "TEAM",
                Ok(vec![group_perm("platform-eng", "Platform Engineering", "read")]),
            )
            .with_group_members("platform-eng", Ok(vec![member("acct-1", "Ada")]));

        let mut conn = open_conn();
        let snapshot_id = collect_and_store(&client, &mut conn, "ws", "2026-01-01T00:00:00Z")
            .await
            .unwrap();

        assert_eq!(client.group_members_call_count("platform-eng"), 1);
        assert_eq!(client.project_group_permissions_call_count("TEAM"), 1);

        let loaded = load_snapshot(&conn, snapshot_id).unwrap();
        assert!(loaded
            .records
            .iter()
            .any(|r| r.scope == crate::model::GrantScope::Project
                && r.access_type == crate::model::AccessType::Group));
        assert!(loaded.records.iter().any(|r| r.scope == crate::model::GrantScope::Project
            && matches!(r.access_type, crate::model::AccessType::Member(_))));
    }

    #[tokio::test]
    async fn project_level_failure_marks_that_project_fetch_failed_only_and_run_continues() {
        let client = FakeBitbucketClient::new(Ok(vec![
            repo("TEAM", "repo-a"),
            repo("OTHER", "repo-b"),
        ]))
        .with_project_direct_permissions("TEAM", Err(ClientError::Other("boom".to_string())))
        .with_project_direct_permissions("OTHER", Ok(vec![user("acct-9", "Static", "read")]))
        .with_direct_permissions("repo-a", Ok(vec![user("acct-1", "Ada", "admin")]));

        let mut conn = open_conn();
        let snapshot_id = collect_and_store(&client, &mut conn, "ws", "2026-01-01T00:00:00Z")
            .await
            .unwrap();

        let loaded = load_snapshot(&conn, snapshot_id).unwrap();
        // repo-a's own RepoStatus is unaffected by its project's fetch failure.
        assert_eq!(
            loaded.repo_statuses.iter().find(|s| s.repo == "repo-a").unwrap().status,
            RepoStatus::Ok
        );
        // No project-scoped record for the failing project, but the direct grant still landed.
        assert!(loaded.records.iter().any(|r| r.repo == "repo-a"
            && r.scope == crate::model::GrantScope::Repo));
        assert!(!loaded
            .records
            .iter()
            .any(|r| r.repo == "repo-a" && r.scope == crate::model::GrantScope::Project));

        let statuses = load_project_fetch_statuses(&conn, snapshot_id).unwrap();
        assert_eq!(
            statuses.iter().find(|s| s.project_key == "TEAM").unwrap().status,
            RepoStatus::FetchFailed
        );
        assert_eq!(
            statuses.iter().find(|s| s.project_key == "OTHER").unwrap().status,
            RepoStatus::Ok
        );
    }

    #[tokio::test]
    async fn unauthorized_on_project_direct_permissions_aborts_the_whole_run() {
        let client = FakeBitbucketClient::new(Ok(vec![repo("TEAM", "repo-a")]))
            .with_project_direct_permissions("TEAM", Err(ClientError::Unauthorized));

        let mut conn = open_conn();
        let result = collect_and_store(&client, &mut conn, "ws", "2026-01-01T00:00:00Z").await;

        assert_eq!(result, Err(RunError::CredentialRejected));
        assert_eq!(snapshot_count(&conn), 0);
    }

    #[tokio::test]
    async fn unauthorized_on_project_group_permissions_aborts_the_whole_run() {
        let client = FakeBitbucketClient::new(Ok(vec![repo("TEAM", "repo-a")]))
            .with_project_group_permissions("TEAM", Err(ClientError::Unauthorized));

        let mut conn = open_conn();
        let result = collect_and_store(&client, &mut conn, "ws", "2026-01-01T00:00:00Z").await;

        assert_eq!(result, Err(RunError::CredentialRejected));
        assert_eq!(snapshot_count(&conn), 0);
    }

    #[tokio::test]
    async fn unauthorized_on_a_project_group_members_call_aborts_the_whole_run() {
        let client = FakeBitbucketClient::new(Ok(vec![repo("TEAM", "repo-a")]))
            .with_project_group_permissions(
                "TEAM",
                Ok(vec![group_perm("locked-group", "Locked Group", "read")]),
            )
            .with_group_members("locked-group", Err(ClientError::Unauthorized));

        let mut conn = open_conn();
        let result = collect_and_store(&client, &mut conn, "ws", "2026-01-01T00:00:00Z").await;

        assert_eq!(result, Err(RunError::CredentialRejected));
        assert_eq!(snapshot_count(&conn), 0);
    }

    #[tokio::test]
    async fn mixed_healthy_and_failing_projects_persist_correctly_in_one_run() {
        let client = FakeBitbucketClient::new(Ok(vec![
            repo("HEALTHY", "repo-a"),
            repo("HEALTHY", "repo-b"),
            repo("FAILING", "repo-c"),
        ]))
        .with_project_direct_permissions("HEALTHY", Ok(vec![user("acct-9", "Static", "read")]))
        .with_project_direct_permissions("FAILING", Err(ClientError::Other("boom".to_string())))
        .with_direct_permissions("repo-c", Ok(vec![user("acct-1", "Ada", "admin")]));

        let mut conn = open_conn();
        let snapshot_id = collect_and_store(&client, &mut conn, "ws", "2026-01-01T00:00:00Z")
            .await
            .unwrap();

        assert_eq!(client.project_direct_permissions_call_count("HEALTHY"), 1);
        assert_eq!(client.project_direct_permissions_call_count("FAILING"), 1);

        let loaded = load_snapshot(&conn, snapshot_id).unwrap();
        let healthy_project_records: Vec<_> = loaded
            .records
            .iter()
            .filter(|r| r.scope == crate::model::GrantScope::Project)
            .collect();
        assert_eq!(healthy_project_records.len(), 2);
        // repo-c's own grant still made it in even though its project failed.
        assert!(loaded.records.iter().any(|r| r.repo == "repo-c"
            && r.scope == crate::model::GrantScope::Repo));
        assert_eq!(
            loaded.repo_statuses.iter().find(|s| s.repo == "repo-c").unwrap().status,
            RepoStatus::Ok
        );

        let statuses = load_project_fetch_statuses(&conn, snapshot_id).unwrap();
        assert_eq!(statuses.len(), 2);
        assert_eq!(
            statuses.iter().find(|s| s.project_key == "HEALTHY").unwrap().status,
            RepoStatus::Ok
        );
        assert_eq!(
            statuses.iter().find(|s| s.project_key == "FAILING").unwrap().status,
            RepoStatus::FetchFailed
        );
    }

    #[tokio::test]
    async fn a_fresh_install_first_run_is_visible_through_list_and_roster_tree() {
        let client = FakeBitbucketClient::new(Ok(vec![repo("TEAM", "repo-a")]))
            .with_direct_permissions("repo-a", Ok(vec![user("acct-1", "Ada", "admin")]));

        let mut conn = open_conn();
        let snapshot_id = collect_and_store(&client, &mut conn, "ws", "2026-01-01T00:00:00Z")
            .await
            .expect("collect_and_store should succeed against a fresh db");

        let snapshots = crate::storage::list_snapshots(&conn).expect("list_snapshots should succeed");
        assert_eq!(
            snapshots.len(),
            1,
            "the snapshot just saved should be visible to list_snapshots, matching what \
             the side drawer's SnapshotsPanel renders"
        );
        assert_eq!(snapshots[0].id, snapshot_id);

        // First-ever run: baseline/comparison both fall back to this one snapshot, exactly
        // as +page.ts computes latestId/previousId when there's only one row.
        let result = crate::roster::get_roster_tree(&conn, snapshot_id, snapshot_id)
            .expect("get_roster_tree should succeed comparing the fresh snapshot to itself");
        assert!(!result.tree.is_empty(), "the roster tree should show the freshly fetched data");
    }

    #[tokio::test]
    async fn a_failure_writing_project_fetch_statuses_rolls_back_the_whole_snapshot() {
        // Regression test for the "data is in the database but the screen shows nothing" bug:
        // project_fetch_statuses used to be written by a separate call *after* save_snapshot's
        // own transaction had already committed, so a failure there left a real, permanent
        // Snapshot behind while collect_and_store still reported the Run as failed — the
        // frontend (RunPanel.svelte), seeing an Err, never navigates/invalidates, so the UI
        // never learns the snapshot exists. Now that project_statuses is written inside
        // save_snapshot's single transaction, the same failure must roll back everything.
        let client = FakeBitbucketClient::new(Ok(vec![repo("TEAM", "repo-a")]))
            .with_direct_permissions("repo-a", Ok(vec![user("acct-1", "Ada", "admin")]));

        let conn = open_conn();
        conn.execute("DROP TABLE project_fetch_statuses", []).unwrap();
        let mut conn = conn;

        let result = collect_and_store(&client, &mut conn, "ws", "2026-01-01T00:00:00Z").await;
        assert!(result.is_err(), "expected the run to report failure");

        let snapshots = crate::storage::list_snapshots(&conn).unwrap();
        assert_eq!(
            snapshots.len(),
            0,
            "a failed Run must never leave a partially-committed Snapshot behind"
        );
    }
}

