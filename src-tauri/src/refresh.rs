//! Orchestrates Refresh (PD-70): after a batch of Pending edits is applied, creates a new
//! Snapshot by copying the source Snapshot's data — the one the batch was staged against — and
//! replacing only the Repo-scope Direct+Group data (records, `RepoFetchStatus`, and
//! `GroupMembershipStatus` rows) for the repos named by the batch's *successful* Repo-scope
//! edits, refetched live via `fetch_repo_permissions` (extracted in PD-69, with fresh
//! group-membership resolution). Everything else — every other repo, every Project-scope
//! record, every other status row — is copied verbatim. Project-scope edits are out of scope
//! for this tracer bullet (PD-68 covers that follow-on). The `refresh_snapshot` Tauri command
//! is a thin wrapper around `refresh_and_store` — all meaningful logic lives here, covered by
//! `cargo test`.

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;

use crate::apply::ApplyResult;
use crate::client::{BitbucketClient, RepoInfo};
use crate::collect::{fetch_repo_permissions, MemberCache};
use crate::diff::Snapshot;
use crate::model::{AccessType, GrantScope};
use crate::storage::{
    load_group_membership_statuses, load_project_fetch_statuses, load_snapshot, save_snapshot,
};

/// Refresh never calls `list_repositories` (ADR-0025), so unlike `RunError` it has no
/// `DiscoveryFailed` counterpart — only a rejected credential (which aborts the whole Refresh,
/// mirroring a Run) or a storage failure can stop it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshError {
    CredentialRejected,
    StorageFailed(String),
}

/// Derives the deduped set of Repo-scope Refresh targets from a batch's *successful* results
/// only — a failed edit contributes no target, and a successful edit outside Repo scope is
/// left for the follow-on Project-scope ticket (PD-68), not attempted here.
fn repo_scope_targets(results: &[ApplyResult]) -> Vec<(String, String)> {
    let mut seen = HashSet::new();
    let mut targets = Vec::new();
    for result in results {
        if result.outcome.is_err() || result.request.scope != GrantScope::Repo {
            continue;
        }
        let key = (result.request.repo_project.clone(), result.request.repo.clone());
        if seen.insert(key.clone()) {
            targets.push(key);
        }
    }
    targets
}

/// Creates a new Snapshot from `source_snapshot_id`, replacing only the Repo-scope Direct+Group
/// data for the repos named by `results`' successful Repo-scope edits with freshly fetched data,
/// and copying every other repo/Project's records and status rows verbatim. Returns the new
/// `snapshots.id`. A 401 on any target aborts the whole Refresh before anything is written; a
/// non-401 failure on one target repo records a `FetchFailed` status (zero records) for that
/// repo only and every other targeted repo still refreshes.
pub async fn refresh_and_store<C: BitbucketClient>(
    client: &C,
    conn: &mut Connection,
    workspace: &str,
    source_snapshot_id: i64,
    run_at: &str,
    results: &[ApplyResult],
) -> Result<i64, RefreshError> {
    let targets = repo_scope_targets(results);

    let source = load_snapshot(conn, source_snapshot_id)
        .map_err(|e| RefreshError::StorageFailed(e.to_string()))?;
    let source_membership_statuses = load_group_membership_statuses(conn, source_snapshot_id)
        .map_err(|e| RefreshError::StorageFailed(e.to_string()))?;
    let project_statuses = load_project_fetch_statuses(conn, source_snapshot_id)
        .map_err(|e| RefreshError::StorageFailed(e.to_string()))?;

    let is_targeted = |repo_project: &str, repo: &str| {
        targets.iter().any(|(p, r)| p == repo_project && r == repo)
    };

    // `GroupMembershipStatus` carries no `scope` field (model.rs) — a Repo-scope and a
    // Project-scope group grant on the same repo produce rows keyed identically on
    // `(repo_project, repo, group_id)`. Filtering `group_membership_statuses` by repo alone
    // would drop a targeted repo's Project-scope group statuses too, even though only its
    // Repo-scope dimension is being refreshed. Scope the drop to just the group ids that were
    // actually Repo-scope `Group` records on that repo in the source Snapshot.
    let repo_scope_group_ids: HashSet<(String, String, String)> = source
        .records
        .iter()
        .filter(|r| r.scope == GrantScope::Repo && r.access_type == AccessType::Group)
        .map(|r| (r.repo_project.clone(), r.repo.clone(), r.principal.id.clone()))
        .collect();

    let mut records: Vec<_> = source
        .records
        .into_iter()
        .filter(|r| !(r.scope == GrantScope::Repo && is_targeted(&r.repo_project, &r.repo)))
        .collect();
    let mut repo_statuses: Vec<_> = source
        .repo_statuses
        .into_iter()
        .filter(|s| !is_targeted(&s.repo_project, &s.repo))
        .collect();
    let mut group_membership_statuses: Vec<_> = source_membership_statuses
        .into_iter()
        .filter(|g| {
            !(is_targeted(&g.repo_project, &g.repo)
                && repo_scope_group_ids.contains(&(
                    g.repo_project.clone(),
                    g.repo.clone(),
                    g.group_id.clone(),
                )))
        })
        .collect();

    // Fresh per-Refresh-call cache (never the source Snapshot's own resolutions) so touched
    // groups' membership is always re-resolved live, mirroring `collect_and_store`'s per-Run
    // cache.
    let mut member_cache: MemberCache = HashMap::new();
    for (repo_project, repo) in &targets {
        let repo_info = RepoInfo { repo_project: repo_project.clone(), repo: repo.clone() };
        let fetch_result = fetch_repo_permissions(client, workspace, &repo_info, &mut member_cache)
            .await
            .map_err(|_| RefreshError::CredentialRejected)?;
        records.extend(fetch_result.records);
        group_membership_statuses.extend(fetch_result.membership_statuses);
        repo_statuses.push(fetch_result.status);
    }

    let snapshot = Snapshot { records, repo_statuses };
    save_snapshot(conn, run_at, &snapshot, &group_membership_statuses, &project_statuses)
        .map_err(|e| RefreshError::StorageFailed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apply::{EditAction, EditTarget, PendingEditRequest};
    use crate::client::ClientError;
    use crate::collect::collect_and_store;
    use crate::model::{AccessType, Permission, RepoStatus};
    use crate::normalize::{RawGroupPermission, RawMember, RawUserPermission};
    use crate::storage::init_schema;
    use std::sync::Mutex;

    struct FakeBitbucketClient {
        repos: Vec<RepoInfo>,
        direct_permissions: HashMap<String, Result<Vec<RawUserPermission>, ClientError>>,
        group_permissions: HashMap<String, Result<Vec<RawGroupPermission>, ClientError>>,
        group_members: HashMap<String, Result<Vec<RawMember>, ClientError>>,
        project_group_permissions: HashMap<String, Result<Vec<RawGroupPermission>, ClientError>>,
        group_members_call_counts: Mutex<HashMap<String, u32>>,
        direct_permissions_call_counts: Mutex<HashMap<String, u32>>,
    }

    impl FakeBitbucketClient {
        fn new() -> Self {
            Self {
                repos: Vec::new(),
                direct_permissions: HashMap::new(),
                group_permissions: HashMap::new(),
                group_members: HashMap::new(),
                project_group_permissions: HashMap::new(),
                group_members_call_counts: Mutex::new(HashMap::new()),
                direct_permissions_call_counts: Mutex::new(HashMap::new()),
            }
        }

        fn with_repos(mut self, repos: Vec<RepoInfo>) -> Self {
            self.repos = repos;
            self
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

        fn with_group_members(mut self, group_slug: &str, result: Result<Vec<RawMember>, ClientError>) -> Self {
            self.group_members.insert(group_slug.to_string(), result);
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

        fn group_members_call_count(&self, group_slug: &str) -> u32 {
            *self.group_members_call_counts.lock().unwrap().get(group_slug).unwrap_or(&0)
        }

        fn direct_permissions_call_count(&self, repo: &str) -> u32 {
            *self.direct_permissions_call_counts.lock().unwrap().get(repo).unwrap_or(&0)
        }
    }

    impl BitbucketClient for FakeBitbucketClient {
        async fn list_repositories(&self, _workspace: &str) -> Result<Vec<RepoInfo>, ClientError> {
            Ok(self.repos.clone())
        }

        async fn list_direct_permissions(
            &self,
            _workspace: &str,
            repo: &str,
        ) -> Result<Vec<RawUserPermission>, ClientError> {
            *self
                .direct_permissions_call_counts
                .lock()
                .unwrap()
                .entry(repo.to_string())
                .or_insert(0) += 1;
            self.direct_permissions.get(repo).cloned().unwrap_or(Ok(Vec::new()))
        }

        async fn list_group_permissions(
            &self,
            _workspace: &str,
            repo: &str,
        ) -> Result<Vec<RawGroupPermission>, ClientError> {
            self.group_permissions.get(repo).cloned().unwrap_or(Ok(Vec::new()))
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
            self.group_members.get(group_slug).cloned().unwrap_or(Ok(Vec::new()))
        }

        async fn list_project_direct_permissions(
            &self,
            _workspace: &str,
            _project_key: &str,
        ) -> Result<Vec<RawUserPermission>, ClientError> {
            Ok(Vec::new())
        }

        async fn list_project_group_permissions(
            &self,
            _workspace: &str,
            project_key: &str,
        ) -> Result<Vec<RawGroupPermission>, ClientError> {
            self.project_group_permissions.get(project_key).cloned().unwrap_or(Ok(Vec::new()))
        }

        async fn set_repo_direct_permission(
            &self,
            _workspace: &str,
            _repo: &str,
            _account_id: &str,
            _permission: Permission,
        ) -> Result<(), ClientError> {
            Ok(())
        }

        async fn remove_repo_direct_permission(
            &self,
            _workspace: &str,
            _repo: &str,
            _account_id: &str,
        ) -> Result<(), ClientError> {
            Ok(())
        }

        async fn set_project_direct_permission(
            &self,
            _workspace: &str,
            _project_key: &str,
            _account_id: &str,
            _permission: Permission,
        ) -> Result<(), ClientError> {
            Ok(())
        }

        async fn remove_project_direct_permission(
            &self,
            _workspace: &str,
            _project_key: &str,
            _account_id: &str,
        ) -> Result<(), ClientError> {
            Ok(())
        }

        async fn set_repo_group_permission(
            &self,
            _workspace: &str,
            _repo: &str,
            _group_slug: &str,
            _permission: Permission,
        ) -> Result<(), ClientError> {
            Ok(())
        }

        async fn remove_repo_group_permission(
            &self,
            _workspace: &str,
            _repo: &str,
            _group_slug: &str,
        ) -> Result<(), ClientError> {
            Ok(())
        }

        async fn set_project_group_permission(
            &self,
            _workspace: &str,
            _project_key: &str,
            _group_slug: &str,
            _permission: Permission,
        ) -> Result<(), ClientError> {
            Ok(())
        }

        async fn remove_project_group_permission(
            &self,
            _workspace: &str,
            _project_key: &str,
            _group_slug: &str,
        ) -> Result<(), ClientError> {
            Ok(())
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

    fn ok_result(repo_project: &str, repo: &str) -> ApplyResult {
        ApplyResult {
            request: PendingEditRequest {
                scope: GrantScope::Repo,
                target: EditTarget::Direct("acct-1".to_string()),
                repo_project: repo_project.to_string(),
                repo: repo.to_string(),
                action: EditAction::SetLevel(Permission::Admin),
            },
            outcome: Ok(()),
        }
    }

    fn failed_result(repo_project: &str, repo: &str) -> ApplyResult {
        ApplyResult {
            request: PendingEditRequest {
                scope: GrantScope::Repo,
                target: EditTarget::Direct("acct-1".to_string()),
                repo_project: repo_project.to_string(),
                repo: repo.to_string(),
                action: EditAction::SetLevel(Permission::Admin),
            },
            outcome: Err(crate::apply::ApplyError::Other("boom".to_string())),
        }
    }

    fn project_scope_ok_result(repo_project: &str, repo: &str) -> ApplyResult {
        ApplyResult {
            request: PendingEditRequest {
                scope: GrantScope::Project,
                target: EditTarget::Direct("acct-1".to_string()),
                repo_project: repo_project.to_string(),
                repo: repo.to_string(),
                action: EditAction::SetLevel(Permission::Admin),
            },
            outcome: Ok(()),
        }
    }

    async fn build_source_snapshot(client: &FakeBitbucketClient, conn: &mut Connection) -> i64 {
        collect_and_store(client, conn, "ws", "2026-01-01T00:00:00Z").await.unwrap()
    }

    #[tokio::test]
    async fn repo_scope_target_refreshes_only_that_repo_and_copies_the_rest_verbatim() {
        let source_client = FakeBitbucketClient::new()
            .with_repos(vec![repo("TEAM", "repo-a"), repo("TEAM", "repo-b")])
            .with_direct_permissions("repo-a", Ok(vec![user("acct-1", "Ada", "read")]))
            .with_direct_permissions("repo-b", Ok(vec![user("acct-2", "Grace", "write")]));

        let mut conn = open_conn();
        let source_id = build_source_snapshot(&source_client, &mut conn).await;

        let refresh_client = FakeBitbucketClient::new()
            .with_direct_permissions("repo-a", Ok(vec![user("acct-1", "Ada", "admin")]));

        let new_id = refresh_and_store(
            &refresh_client,
            &mut conn,
            "ws",
            source_id,
            "2026-01-02T00:00:00Z",
            &[ok_result("TEAM", "repo-a")],
        )
        .await
        .unwrap();

        let loaded = load_snapshot(&conn, new_id).unwrap();
        let repo_a_record = loaded.records.iter().find(|r| r.repo == "repo-a").unwrap();
        assert_eq!(repo_a_record.permission, Permission::Admin);

        // repo-b is untouched — its record is copied verbatim from the source Snapshot.
        let repo_b_record = loaded.records.iter().find(|r| r.repo == "repo-b").unwrap();
        assert_eq!(repo_b_record.permission, Permission::Write);
        assert_eq!(refresh_client.direct_permissions_call_count("repo-b"), 0);
    }

    #[tokio::test]
    async fn duplicate_targets_in_one_batch_are_fetched_exactly_once() {
        let source_client = FakeBitbucketClient::new().with_repos(vec![repo("TEAM", "repo-a")]);
        let mut conn = open_conn();
        let source_id = build_source_snapshot(&source_client, &mut conn).await;

        let refresh_client = FakeBitbucketClient::new()
            .with_direct_permissions("repo-a", Ok(vec![user("acct-1", "Ada", "admin")]));

        let new_id = refresh_and_store(
            &refresh_client,
            &mut conn,
            "ws",
            source_id,
            "2026-01-02T00:00:00Z",
            &[ok_result("TEAM", "repo-a"), ok_result("TEAM", "repo-a")],
        )
        .await
        .unwrap();

        assert_eq!(refresh_client.direct_permissions_call_count("repo-a"), 1);
        let loaded = load_snapshot(&conn, new_id).unwrap();
        assert_eq!(loaded.repo_statuses.iter().filter(|s| s.repo == "repo-a").count(), 1);
    }

    #[tokio::test]
    async fn failed_results_contribute_no_target_and_are_excluded_from_the_fetch() {
        let source_client = FakeBitbucketClient::new()
            .with_repos(vec![repo("TEAM", "repo-a")])
            .with_direct_permissions("repo-a", Ok(vec![user("acct-1", "Ada", "read")]));
        let mut conn = open_conn();
        let source_id = build_source_snapshot(&source_client, &mut conn).await;

        let refresh_client = FakeBitbucketClient::new()
            .with_direct_permissions("repo-a", Ok(vec![user("acct-1", "Ada", "admin")]));

        let new_id = refresh_and_store(
            &refresh_client,
            &mut conn,
            "ws",
            source_id,
            "2026-01-02T00:00:00Z",
            &[failed_result("TEAM", "repo-a")],
        )
        .await
        .unwrap();

        assert_eq!(refresh_client.direct_permissions_call_count("repo-a"), 0);
        let loaded = load_snapshot(&conn, new_id).unwrap();
        // Copied verbatim from the source — the failed edit never touched it.
        assert_eq!(loaded.records.iter().find(|r| r.repo == "repo-a").unwrap().permission, Permission::Read);
    }

    #[tokio::test]
    async fn a_targeted_repos_project_scope_group_membership_status_is_untouched() {
        // Regression test: `GroupMembershipStatus` carries no `scope` field, so a Repo-scope
        // and a Project-scope group grant on the same repo produce rows keyed identically on
        // (repo_project, repo, group_id). A targeted repo's Repo-scope refresh must not drop
        // its Project-scope group's membership-status row too.
        let source_client = FakeBitbucketClient::new()
            .with_repos(vec![repo("TEAM", "repo-a")])
            .with_group_permissions(
                "repo-a",
                Ok(vec![RawGroupPermission {
                    group_slug: "repo-group".to_string(),
                    group_name: "Repo Group".to_string(),
                    permission: "write".to_string(),
                    members: crate::normalize::RawGroupMembersResponse::FetchFailed,
                }]),
            )
            .with_project_group_permissions(
                "TEAM",
                Ok(vec![RawGroupPermission {
                    group_slug: "project-group".to_string(),
                    group_name: "Project Group".to_string(),
                    permission: "read".to_string(),
                    members: crate::normalize::RawGroupMembersResponse::FetchFailed,
                }]),
            )
            .with_group_members("repo-group", Ok(vec![RawMember {
                account_id: "acct-1".to_string(),
                display_name: "Ada".to_string(),
            }]))
            .with_group_members("project-group", Ok(vec![RawMember {
                account_id: "acct-2".to_string(),
                display_name: "Grace".to_string(),
            }]));
        let mut conn = open_conn();
        let source_id = build_source_snapshot(&source_client, &mut conn).await;
        let source_statuses = load_group_membership_statuses(&conn, source_id).unwrap();
        assert_eq!(source_statuses.len(), 2, "sanity check: both groups resolved on repo-a");

        let refresh_client = FakeBitbucketClient::new().with_group_permissions(
            "repo-a",
            Ok(vec![RawGroupPermission {
                group_slug: "repo-group".to_string(),
                group_name: "Repo Group".to_string(),
                permission: "admin".to_string(),
                members: crate::normalize::RawGroupMembersResponse::FetchFailed,
            }]),
        );

        let new_id = refresh_and_store(
            &refresh_client,
            &mut conn,
            "ws",
            source_id,
            "2026-01-02T00:00:00Z",
            &[ok_result("TEAM", "repo-a")],
        )
        .await
        .unwrap();

        let new_statuses = load_group_membership_statuses(&conn, new_id).unwrap();
        // The Project-scope group's membership status survives untouched...
        assert!(new_statuses.iter().any(|s| s.group_id == "project-group" && s.members_resolved));
        // ...while the Repo-scope group's status was refreshed (still present, just live).
        assert!(new_statuses.iter().any(|s| s.group_id == "repo-group"));
        assert_eq!(new_statuses.len(), 2);

        let loaded = load_snapshot(&conn, new_id).unwrap();
        let repo_group_record = loaded
            .records
            .iter()
            .find(|r| r.scope == GrantScope::Repo && r.access_type == AccessType::Group)
            .unwrap();
        assert_eq!(repo_group_record.permission, Permission::Admin);
        // The Project-scope group's own record is untouched, carried over from the source.
        let project_group_record = loaded
            .records
            .iter()
            .find(|r| r.scope == GrantScope::Project && r.access_type == AccessType::Group)
            .unwrap();
        assert_eq!(project_group_record.permission, Permission::Read);
    }

    #[tokio::test]
    async fn a_401_on_any_target_aborts_the_whole_refresh_with_no_snapshot_written() {
        let source_client = FakeBitbucketClient::new().with_repos(vec![repo("TEAM", "repo-a")]);
        let mut conn = open_conn();
        let source_id = build_source_snapshot(&source_client, &mut conn).await;

        let refresh_client = FakeBitbucketClient::new()
            .with_direct_permissions("repo-a", Err(ClientError::Unauthorized));

        let snapshots_before: i64 =
            conn.query_row("SELECT COUNT(*) FROM snapshots", [], |r| r.get(0)).unwrap();

        let result = refresh_and_store(
            &refresh_client,
            &mut conn,
            "ws",
            source_id,
            "2026-01-02T00:00:00Z",
            &[ok_result("TEAM", "repo-a")],
        )
        .await;

        assert_eq!(result, Err(RefreshError::CredentialRejected));
        let snapshots_after: i64 =
            conn.query_row("SELECT COUNT(*) FROM snapshots", [], |r| r.get(0)).unwrap();
        assert_eq!(snapshots_before, snapshots_after);
    }

    #[tokio::test]
    async fn a_non_401_failure_on_one_target_records_fetch_failed_and_the_rest_completes() {
        let source_client = FakeBitbucketClient::new()
            .with_repos(vec![repo("TEAM", "repo-a"), repo("TEAM", "repo-b")]);
        let mut conn = open_conn();
        let source_id = build_source_snapshot(&source_client, &mut conn).await;

        let refresh_client = FakeBitbucketClient::new()
            .with_direct_permissions("repo-a", Err(ClientError::Other("boom".to_string())))
            .with_direct_permissions("repo-b", Ok(vec![user("acct-2", "Grace", "admin")]));

        let new_id = refresh_and_store(
            &refresh_client,
            &mut conn,
            "ws",
            source_id,
            "2026-01-02T00:00:00Z",
            &[ok_result("TEAM", "repo-a"), ok_result("TEAM", "repo-b")],
        )
        .await
        .unwrap();

        let loaded = load_snapshot(&conn, new_id).unwrap();
        assert_eq!(
            loaded.repo_statuses.iter().find(|s| s.repo == "repo-a").unwrap().status,
            RepoStatus::FetchFailed
        );
        assert!(!loaded.records.iter().any(|r| r.repo == "repo-a"));
        assert_eq!(
            loaded.repo_statuses.iter().find(|s| s.repo == "repo-b").unwrap().status,
            RepoStatus::Ok
        );
        assert!(loaded.records.iter().any(|r| r.repo == "repo-b" && r.permission == Permission::Admin));
    }

    #[tokio::test]
    async fn group_membership_is_resolved_live_not_copied_from_the_source_snapshot() {
        let source_client = FakeBitbucketClient::new()
            .with_repos(vec![repo("TEAM", "repo-a")])
            .with_group_permissions("repo-a", Ok(vec![RawGroupPermission {
                group_slug: "platform-eng".to_string(),
                group_name: "Platform Engineering".to_string(),
                permission: "write".to_string(),
                members: crate::normalize::RawGroupMembersResponse::FetchFailed,
            }]))
            .with_group_members("platform-eng", Ok(vec![RawMember {
                account_id: "acct-1".to_string(),
                display_name: "Ada".to_string(),
            }]));
        let mut conn = open_conn();
        let source_id = build_source_snapshot(&source_client, &mut conn).await;
        assert_eq!(source_client.group_members_call_count("platform-eng"), 1);

        let refresh_client = FakeBitbucketClient::new().with_group_permissions(
            "repo-a",
            Ok(vec![RawGroupPermission {
                group_slug: "platform-eng".to_string(),
                group_name: "Platform Engineering".to_string(),
                permission: "admin".to_string(),
                members: crate::normalize::RawGroupMembersResponse::FetchFailed,
            }]),
        );
        // No group_members fixture on refresh_client — defaults to `Ok(vec![])`, proving the
        // membership call actually happened live rather than reusing the source's members.

        let new_id = refresh_and_store(
            &refresh_client,
            &mut conn,
            "ws",
            source_id,
            "2026-01-02T00:00:00Z",
            &[ok_result("TEAM", "repo-a")],
        )
        .await
        .unwrap();

        assert_eq!(refresh_client.group_members_call_count("platform-eng"), 1);
        let loaded = load_snapshot(&conn, new_id).unwrap();
        let group_record = loaded
            .records
            .iter()
            .find(|r| r.repo == "repo-a" && r.access_type == AccessType::Group)
            .unwrap();
        assert_eq!(group_record.permission, Permission::Admin);
        // The live-but-empty membership resolution means no Member record this time, unlike
        // the source Snapshot which had one.
        assert!(!loaded.records.iter().any(|r| matches!(r.access_type, AccessType::Member(_))));
    }

    #[tokio::test]
    async fn a_batch_with_zero_eligible_targets_produces_an_exact_copy_of_the_source() {
        let source_client = FakeBitbucketClient::new()
            .with_repos(vec![repo("TEAM", "repo-a")])
            .with_direct_permissions("repo-a", Ok(vec![user("acct-1", "Ada", "read")]));
        let mut conn = open_conn();
        let source_id = build_source_snapshot(&source_client, &mut conn).await;
        let source_snapshot = load_snapshot(&conn, source_id).unwrap();

        let refresh_client = FakeBitbucketClient::new();

        // An empty results slice, and a batch whose only result is Project-scope (out of
        // scope for this tracer bullet, PD-70), both produce zero eligible targets.
        for results in [Vec::new(), vec![project_scope_ok_result("TEAM", "repo-a")]] {
            let new_id = refresh_and_store(
                &refresh_client,
                &mut conn,
                "ws",
                source_id,
                "2026-01-02T00:00:00Z",
                &results,
            )
            .await
            .unwrap();

            let loaded = load_snapshot(&conn, new_id).unwrap();
            assert_eq!(loaded.records, source_snapshot.records);
            assert_eq!(loaded.repo_statuses, source_snapshot.repo_statuses);
        }
        assert_eq!(refresh_client.direct_permissions_call_count("repo-a"), 0);
    }
}
