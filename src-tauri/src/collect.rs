//! Orchestrates one Run: discovers repos, fetches each repo's Direct and Group grants,
//! normalizes them via the existing PD-2 `normalize_repo_permissions` and PD-3
//! `normalize_repo_group_permissions` unmodified, and persists the result as one new
//! immutable Snapshot. The `run_now` Tauri command is a thin wrapper around
//! `collect_and_store` — all meaningful logic lives here, covered by `cargo test`.

use rusqlite::Connection;

use crate::client::{BitbucketClient, ClientError};
use crate::diff::Snapshot;
use crate::model::{PermissionRecord, RepoFetchStatus, RepoStatus};
use crate::normalize::{
    normalize_repo_group_permissions, normalize_repo_permissions, RawGroupsResponse,
    RawUsersResponse,
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
            Ok(groups) => RawGroupsResponse::Ok(groups),
            Err(ClientError::Unauthorized) => return Err(RunError::CredentialRejected),
            Err(ClientError::RateLimited) | Err(ClientError::Other(_)) => {
                RawGroupsResponse::FetchFailed
            }
        };
        let group_fetch_failed = matches!(raw_groups, RawGroupsResponse::FetchFailed);
        // Membership statuses are always unresolved until PD-9 wires up member fetching, so
        // persisting them is that ticket's job, not this one's.
        let (mut group_records, _membership_statuses) =
            normalize_repo_group_permissions(&repo.repo_project, &repo.repo, raw_groups);
        records.append(&mut group_records);

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
    save_snapshot(conn, run_at, &snapshot, &[])
        .map_err(|e| RunError::StorageFailed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::RepoInfo;
    use crate::model::RepoStatus;
    use crate::normalize::{RawGroupMembersResponse, RawGroupPermission, RawUserPermission};
    use crate::storage::{init_schema, load_snapshot};
    use std::collections::HashMap;

    struct FakeBitbucketClient {
        repos: Result<Vec<RepoInfo>, ClientError>,
        direct_permissions: HashMap<String, Result<Vec<RawUserPermission>, ClientError>>,
        group_permissions: HashMap<String, Result<Vec<RawGroupPermission>, ClientError>>,
    }

    impl FakeBitbucketClient {
        fn new(repos: Result<Vec<RepoInfo>, ClientError>) -> Self {
            Self {
                repos,
                direct_permissions: HashMap::new(),
                group_permissions: HashMap::new(),
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
}
