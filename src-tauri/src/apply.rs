//! Applies a batch of staged permission edits (PD-59/PD-60/PD-61/PD-62) to live Bitbucket. Each
//! `PendingEditRequest` is routed to the matching `BitbucketClient` write method and applied
//! independently (ADR-0022) — one item failing never blocks the rest of the batch. All four
//! `(scope, target)` combinations — Direct/Repo (PD-60), Direct/Project (PD-61), Group/Repo and
//! Group/Project (PD-62) — are wired to real client calls.

use serde::{Deserialize, Serialize};

use crate::client::{BitbucketClient, ClientError};
use crate::model::{GrantScope, Permission};

/// Who the edit targets. `Direct` carries a Bitbucket `account_id`; `Group` a group slug —
/// mirrors `AccessType`'s identity fields (model.rs) without pulling in `Member`, since a
/// member's access can never be edited directly (ADR-0021).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "id")]
pub enum EditTarget {
    Direct(String),
    Group(String),
}

/// What to do to the targeted grant. `SetLevel` never carries `Permission::CreateRepo` in
/// practice — the frontend's level picker only ever offers Read/Write/Admin (ADR-0021) — but
/// this stays the full `Permission` type rather than a narrower one so it doesn't need its own
/// parallel enum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "level")]
pub enum EditAction {
    SetLevel(Permission),
    Remove,
}

/// One staged edit, exactly as the frontend's `pendingEdits.ts` sends it to the
/// `apply_pending_edits` Tauri command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingEditRequest {
    pub scope: GrantScope,
    pub target: EditTarget,
    pub repo_project: String,
    pub repo: String,
    pub action: EditAction,
}

/// Mirrors `ClientError` (per-item, so the frontend can render "insufficient scope" vs.
/// "rate limited" vs. an opaque failure differently) without reusing that type directly —
/// `ClientError` isn't `Serialize` and belongs to the read-path abstraction, not the wire
/// contract this command exposes to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", content = "message")]
pub enum ApplyError {
    Unauthorized,
    RateLimited,
    Other(String),
}

impl From<ClientError> for ApplyError {
    fn from(e: ClientError) -> Self {
        match e {
            ClientError::Unauthorized => ApplyError::Unauthorized,
            ClientError::RateLimited => ApplyError::RateLimited,
            ClientError::Other(msg) => ApplyError::Other(msg),
        }
    }
}

/// One edit's outcome, paired with the request it came from so the frontend can match a result
/// back to its staged edit without relying on array-order bookkeeping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    pub request: PendingEditRequest,
    pub outcome: Result<(), ApplyError>,
}

/// Routes one edit to the matching `BitbucketClient` write method. `Project` scope targets
/// `edit.repo_project` as the project key, cascading to every repo it owns, rather than
/// `edit.repo` — true for both `Direct` (PD-61) and `Group` (PD-62) targets.
async fn apply_one<C: BitbucketClient>(
    client: &C,
    workspace: &str,
    edit: &PendingEditRequest,
) -> Result<(), ApplyError> {
    match (edit.scope, &edit.target) {
        (GrantScope::Repo, EditTarget::Direct(account_id)) => match &edit.action {
            EditAction::SetLevel(permission) => client
                .set_repo_direct_permission(workspace, &edit.repo, account_id, *permission)
                .await
                .map_err(ApplyError::from),
            EditAction::Remove => client
                .remove_repo_direct_permission(workspace, &edit.repo, account_id)
                .await
                .map_err(ApplyError::from),
        },
        (GrantScope::Project, EditTarget::Direct(account_id)) => match &edit.action {
            EditAction::SetLevel(permission) => client
                .set_project_direct_permission(workspace, &edit.repo_project, account_id, *permission)
                .await
                .map_err(ApplyError::from),
            EditAction::Remove => client
                .remove_project_direct_permission(workspace, &edit.repo_project, account_id)
                .await
                .map_err(ApplyError::from),
        },
        (GrantScope::Repo, EditTarget::Group(group_slug)) => match &edit.action {
            EditAction::SetLevel(permission) => client
                .set_repo_group_permission(workspace, &edit.repo, group_slug, *permission)
                .await
                .map_err(ApplyError::from),
            EditAction::Remove => client
                .remove_repo_group_permission(workspace, &edit.repo, group_slug)
                .await
                .map_err(ApplyError::from),
        },
        (GrantScope::Project, EditTarget::Group(group_slug)) => match &edit.action {
            EditAction::SetLevel(permission) => client
                .set_project_group_permission(workspace, &edit.repo_project, group_slug, *permission)
                .await
                .map_err(ApplyError::from),
            EditAction::Remove => client
                .remove_project_group_permission(workspace, &edit.repo_project, group_slug)
                .await
                .map_err(ApplyError::from),
        },
    }
}

/// Applies every edit in `edits` independently (ADR-0022) — a failure on one item doesn't stop
/// the batch — and returns one `ApplyResult` per edit, in the same order they were given.
pub async fn apply_pending_edits<C: BitbucketClient>(
    client: &C,
    workspace: &str,
    edits: Vec<PendingEditRequest>,
) -> Vec<ApplyResult> {
    let mut results = Vec::with_capacity(edits.len());
    for edit in edits {
        let outcome = apply_one(client, workspace, &edit).await;
        results.push(ApplyResult { request: edit, outcome });
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::RepoInfo;
    use crate::normalize::{RawGroupPermission, RawMember, RawUserPermission};
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct SetCall {
        workspace: String,
        repo: String,
        account_id: String,
        permission: Permission,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct RemoveCall {
        workspace: String,
        repo: String,
        account_id: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct ProjectSetCall {
        workspace: String,
        project_key: String,
        account_id: String,
        permission: Permission,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct ProjectRemoveCall {
        workspace: String,
        project_key: String,
        account_id: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct GroupSetCall {
        workspace: String,
        repo: String,
        group_slug: String,
        permission: Permission,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct GroupRemoveCall {
        workspace: String,
        repo: String,
        group_slug: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct ProjectGroupSetCall {
        workspace: String,
        project_key: String,
        group_slug: String,
        permission: Permission,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct ProjectGroupRemoveCall {
        workspace: String,
        project_key: String,
        group_slug: String,
    }

    /// Follows `collect.rs`'s `FakeBitbucketClient` pattern: canned per-key results plus call
    /// tracking, so tests assert externally observable behaviour (which method was called, with
    /// what arguments, and what `ApplyResult` came back) rather than internal control flow. The
    /// read-side methods are never exercised by these tests, so they return trivial empty
    /// results just to satisfy the trait.
    #[derive(Default)]
    struct FakeClient {
        set_results: HashMap<String, Result<(), ClientError>>,
        remove_results: HashMap<String, Result<(), ClientError>>,
        project_set_results: HashMap<String, Result<(), ClientError>>,
        project_remove_results: HashMap<String, Result<(), ClientError>>,
        group_set_results: HashMap<String, Result<(), ClientError>>,
        group_remove_results: HashMap<String, Result<(), ClientError>>,
        project_group_set_results: HashMap<String, Result<(), ClientError>>,
        project_group_remove_results: HashMap<String, Result<(), ClientError>>,
        set_calls: Mutex<Vec<SetCall>>,
        remove_calls: Mutex<Vec<RemoveCall>>,
        project_set_calls: Mutex<Vec<ProjectSetCall>>,
        project_remove_calls: Mutex<Vec<ProjectRemoveCall>>,
        group_set_calls: Mutex<Vec<GroupSetCall>>,
        group_remove_calls: Mutex<Vec<GroupRemoveCall>>,
        project_group_set_calls: Mutex<Vec<ProjectGroupSetCall>>,
        project_group_remove_calls: Mutex<Vec<ProjectGroupRemoveCall>>,
    }

    impl FakeClient {
        fn new() -> Self {
            Self::default()
        }

        fn with_set_result(mut self, account_id: &str, result: Result<(), ClientError>) -> Self {
            self.set_results.insert(account_id.to_string(), result);
            self
        }

        fn with_remove_result(mut self, account_id: &str, result: Result<(), ClientError>) -> Self {
            self.remove_results.insert(account_id.to_string(), result);
            self
        }

        fn with_project_set_result(mut self, account_id: &str, result: Result<(), ClientError>) -> Self {
            self.project_set_results.insert(account_id.to_string(), result);
            self
        }

        fn with_project_remove_result(mut self, account_id: &str, result: Result<(), ClientError>) -> Self {
            self.project_remove_results.insert(account_id.to_string(), result);
            self
        }

        fn with_group_set_result(mut self, group_slug: &str, result: Result<(), ClientError>) -> Self {
            self.group_set_results.insert(group_slug.to_string(), result);
            self
        }

        fn with_group_remove_result(mut self, group_slug: &str, result: Result<(), ClientError>) -> Self {
            self.group_remove_results.insert(group_slug.to_string(), result);
            self
        }

        fn with_project_group_set_result(mut self, group_slug: &str, result: Result<(), ClientError>) -> Self {
            self.project_group_set_results.insert(group_slug.to_string(), result);
            self
        }

        fn with_project_group_remove_result(mut self, group_slug: &str, result: Result<(), ClientError>) -> Self {
            self.project_group_remove_results.insert(group_slug.to_string(), result);
            self
        }
    }

    impl BitbucketClient for FakeClient {
        async fn list_repositories(&self, _workspace: &str) -> Result<Vec<RepoInfo>, ClientError> {
            Ok(vec![])
        }
        async fn list_direct_permissions(
            &self,
            _workspace: &str,
            _repo: &str,
        ) -> Result<Vec<RawUserPermission>, ClientError> {
            Ok(vec![])
        }
        async fn list_group_permissions(
            &self,
            _workspace: &str,
            _repo: &str,
        ) -> Result<Vec<RawGroupPermission>, ClientError> {
            Ok(vec![])
        }
        async fn list_group_members(
            &self,
            _workspace: &str,
            _group_slug: &str,
        ) -> Result<Vec<RawMember>, ClientError> {
            Ok(vec![])
        }
        async fn list_project_direct_permissions(
            &self,
            _workspace: &str,
            _project_key: &str,
        ) -> Result<Vec<RawUserPermission>, ClientError> {
            Ok(vec![])
        }
        async fn list_project_group_permissions(
            &self,
            _workspace: &str,
            _project_key: &str,
        ) -> Result<Vec<RawGroupPermission>, ClientError> {
            Ok(vec![])
        }

        async fn set_repo_direct_permission(
            &self,
            workspace: &str,
            repo: &str,
            account_id: &str,
            permission: Permission,
        ) -> Result<(), ClientError> {
            self.set_calls.lock().unwrap().push(SetCall {
                workspace: workspace.to_string(),
                repo: repo.to_string(),
                account_id: account_id.to_string(),
                permission,
            });
            self.set_results.get(account_id).cloned().unwrap_or(Ok(()))
        }

        async fn remove_repo_direct_permission(
            &self,
            workspace: &str,
            repo: &str,
            account_id: &str,
        ) -> Result<(), ClientError> {
            self.remove_calls.lock().unwrap().push(RemoveCall {
                workspace: workspace.to_string(),
                repo: repo.to_string(),
                account_id: account_id.to_string(),
            });
            self.remove_results.get(account_id).cloned().unwrap_or(Ok(()))
        }

        async fn set_project_direct_permission(
            &self,
            workspace: &str,
            project_key: &str,
            account_id: &str,
            permission: Permission,
        ) -> Result<(), ClientError> {
            self.project_set_calls.lock().unwrap().push(ProjectSetCall {
                workspace: workspace.to_string(),
                project_key: project_key.to_string(),
                account_id: account_id.to_string(),
                permission,
            });
            self.project_set_results.get(account_id).cloned().unwrap_or(Ok(()))
        }

        async fn remove_project_direct_permission(
            &self,
            workspace: &str,
            project_key: &str,
            account_id: &str,
        ) -> Result<(), ClientError> {
            self.project_remove_calls.lock().unwrap().push(ProjectRemoveCall {
                workspace: workspace.to_string(),
                project_key: project_key.to_string(),
                account_id: account_id.to_string(),
            });
            self.project_remove_results.get(account_id).cloned().unwrap_or(Ok(()))
        }

        async fn set_repo_group_permission(
            &self,
            workspace: &str,
            repo: &str,
            group_slug: &str,
            permission: Permission,
        ) -> Result<(), ClientError> {
            self.group_set_calls.lock().unwrap().push(GroupSetCall {
                workspace: workspace.to_string(),
                repo: repo.to_string(),
                group_slug: group_slug.to_string(),
                permission,
            });
            self.group_set_results.get(group_slug).cloned().unwrap_or(Ok(()))
        }

        async fn remove_repo_group_permission(
            &self,
            workspace: &str,
            repo: &str,
            group_slug: &str,
        ) -> Result<(), ClientError> {
            self.group_remove_calls.lock().unwrap().push(GroupRemoveCall {
                workspace: workspace.to_string(),
                repo: repo.to_string(),
                group_slug: group_slug.to_string(),
            });
            self.group_remove_results.get(group_slug).cloned().unwrap_or(Ok(()))
        }

        async fn set_project_group_permission(
            &self,
            workspace: &str,
            project_key: &str,
            group_slug: &str,
            permission: Permission,
        ) -> Result<(), ClientError> {
            self.project_group_set_calls.lock().unwrap().push(ProjectGroupSetCall {
                workspace: workspace.to_string(),
                project_key: project_key.to_string(),
                group_slug: group_slug.to_string(),
                permission,
            });
            self.project_group_set_results.get(group_slug).cloned().unwrap_or(Ok(()))
        }

        async fn remove_project_group_permission(
            &self,
            workspace: &str,
            project_key: &str,
            group_slug: &str,
        ) -> Result<(), ClientError> {
            self.project_group_remove_calls.lock().unwrap().push(ProjectGroupRemoveCall {
                workspace: workspace.to_string(),
                project_key: project_key.to_string(),
                group_slug: group_slug.to_string(),
            });
            self.project_group_remove_results.get(group_slug).cloned().unwrap_or(Ok(()))
        }
    }

    fn set_level_edit(repo: &str, account_id: &str, permission: Permission) -> PendingEditRequest {
        PendingEditRequest {
            scope: GrantScope::Repo,
            target: EditTarget::Direct(account_id.to_string()),
            repo_project: "TEAM".to_string(),
            repo: repo.to_string(),
            action: EditAction::SetLevel(permission),
        }
    }

    fn remove_edit(repo: &str, account_id: &str) -> PendingEditRequest {
        PendingEditRequest {
            scope: GrantScope::Repo,
            target: EditTarget::Direct(account_id.to_string()),
            repo_project: "TEAM".to_string(),
            repo: repo.to_string(),
            action: EditAction::Remove,
        }
    }

    fn set_level_project_edit(project_key: &str, account_id: &str, permission: Permission) -> PendingEditRequest {
        PendingEditRequest {
            scope: GrantScope::Project,
            target: EditTarget::Direct(account_id.to_string()),
            repo_project: project_key.to_string(),
            repo: "repo-a".to_string(),
            action: EditAction::SetLevel(permission),
        }
    }

    fn remove_project_edit(project_key: &str, account_id: &str) -> PendingEditRequest {
        PendingEditRequest {
            scope: GrantScope::Project,
            target: EditTarget::Direct(account_id.to_string()),
            repo_project: project_key.to_string(),
            repo: "repo-a".to_string(),
            action: EditAction::Remove,
        }
    }

    fn set_level_group_edit(repo: &str, group_slug: &str, permission: Permission) -> PendingEditRequest {
        PendingEditRequest {
            scope: GrantScope::Repo,
            target: EditTarget::Group(group_slug.to_string()),
            repo_project: "TEAM".to_string(),
            repo: repo.to_string(),
            action: EditAction::SetLevel(permission),
        }
    }

    fn remove_group_edit(repo: &str, group_slug: &str) -> PendingEditRequest {
        PendingEditRequest {
            scope: GrantScope::Repo,
            target: EditTarget::Group(group_slug.to_string()),
            repo_project: "TEAM".to_string(),
            repo: repo.to_string(),
            action: EditAction::Remove,
        }
    }

    fn set_level_project_group_edit(project_key: &str, group_slug: &str, permission: Permission) -> PendingEditRequest {
        PendingEditRequest {
            scope: GrantScope::Project,
            target: EditTarget::Group(group_slug.to_string()),
            repo_project: project_key.to_string(),
            repo: "repo-a".to_string(),
            action: EditAction::SetLevel(permission),
        }
    }

    fn remove_project_group_edit(project_key: &str, group_slug: &str) -> PendingEditRequest {
        PendingEditRequest {
            scope: GrantScope::Project,
            target: EditTarget::Group(group_slug.to_string()),
            repo_project: project_key.to_string(),
            repo: "repo-a".to_string(),
            action: EditAction::Remove,
        }
    }

    #[tokio::test]
    async fn a_set_level_edit_routes_to_set_repo_direct_permission_with_the_right_arguments() {
        let client = FakeClient::new();
        let edits = vec![set_level_edit("repo-a", "acct-1", Permission::Admin)];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].outcome, Ok(()));
        let calls = client.set_calls.lock().unwrap();
        assert_eq!(
            calls[0],
            SetCall {
                workspace: "ws".to_string(),
                repo: "repo-a".to_string(),
                account_id: "acct-1".to_string(),
                permission: Permission::Admin,
            }
        );
        assert!(client.remove_calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_remove_edit_routes_to_remove_repo_direct_permission_with_the_right_arguments() {
        let client = FakeClient::new();
        let edits = vec![remove_edit("repo-a", "acct-1")];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].outcome, Ok(()));
        let calls = client.remove_calls.lock().unwrap();
        assert_eq!(
            calls[0],
            RemoveCall { workspace: "ws".to_string(), repo: "repo-a".to_string(), account_id: "acct-1".to_string() }
        );
        assert!(client.set_calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_batch_with_one_failing_item_still_applies_and_reports_the_rest() {
        let client = FakeClient::new()
            .with_set_result("acct-fail", Err(ClientError::Other("boom".to_string())));
        let edits = vec![
            set_level_edit("repo-a", "acct-fail", Permission::Write),
            set_level_edit("repo-b", "acct-ok", Permission::Read),
        ];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].outcome, Err(ApplyError::Other("boom".to_string())));
        assert_eq!(results[1].outcome, Ok(()));
        // Both calls still happened — the first failing did not block the second.
        assert_eq!(client.set_calls.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn unauthorized_and_rate_limited_client_errors_surface_as_distinct_typed_results() {
        let client = FakeClient::new()
            .with_set_result("acct-1", Err(ClientError::Unauthorized))
            .with_remove_result("acct-2", Err(ClientError::RateLimited));
        let edits = vec![
            set_level_edit("repo-a", "acct-1", Permission::Admin),
            remove_edit("repo-a", "acct-2"),
        ];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results[0].outcome, Err(ApplyError::Unauthorized));
        assert_eq!(results[1].outcome, Err(ApplyError::RateLimited));
    }

    #[tokio::test]
    async fn an_unauthorized_item_does_not_abort_the_rest_of_the_batch() {
        // Unlike collect_and_store's Run-wide abort-on-401 (PD-7), apply_pending_edits treats
        // every item independently (ADR-0022) — permission edits are user-initiated one-offs,
        // not a single atomic Run.
        let client = FakeClient::new().with_set_result("acct-1", Err(ClientError::Unauthorized));
        let edits = vec![
            set_level_edit("repo-a", "acct-1", Permission::Admin),
            set_level_edit("repo-b", "acct-2", Permission::Read),
        ];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results[0].outcome, Err(ApplyError::Unauthorized));
        assert_eq!(results[1].outcome, Ok(()));
    }

    #[tokio::test]
    async fn a_group_set_level_edit_routes_to_set_repo_group_permission_with_the_right_arguments() {
        let client = FakeClient::new();
        let edits = vec![set_level_group_edit("repo-a", "platform-eng", Permission::Write)];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].outcome, Ok(()));
        let calls = client.group_set_calls.lock().unwrap();
        assert_eq!(
            calls[0],
            GroupSetCall {
                workspace: "ws".to_string(),
                repo: "repo-a".to_string(),
                group_slug: "platform-eng".to_string(),
                permission: Permission::Write,
            }
        );
        assert!(client.group_remove_calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_group_remove_edit_routes_to_remove_repo_group_permission_with_the_right_arguments() {
        let client = FakeClient::new();
        let edits = vec![remove_group_edit("repo-a", "platform-eng")];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].outcome, Ok(()));
        let calls = client.group_remove_calls.lock().unwrap();
        assert_eq!(
            calls[0],
            GroupRemoveCall {
                workspace: "ws".to_string(),
                repo: "repo-a".to_string(),
                group_slug: "platform-eng".to_string(),
            }
        );
        assert!(client.group_set_calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_project_scoped_group_set_level_edit_routes_to_set_project_group_permission_with_the_right_arguments() {
        let client = FakeClient::new();
        let edits = vec![set_level_project_group_edit("TEAM", "platform-eng", Permission::Admin)];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].outcome, Ok(()));
        let calls = client.project_group_set_calls.lock().unwrap();
        assert_eq!(
            calls[0],
            ProjectGroupSetCall {
                workspace: "ws".to_string(),
                project_key: "TEAM".to_string(),
                group_slug: "platform-eng".to_string(),
                permission: Permission::Admin,
            }
        );
        assert!(client.project_group_remove_calls.lock().unwrap().is_empty());
        assert!(client.group_set_calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_project_scoped_group_remove_edit_routes_to_remove_project_group_permission_with_the_right_arguments() {
        let client = FakeClient::new();
        let edits = vec![remove_project_group_edit("TEAM", "platform-eng")];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].outcome, Ok(()));
        let calls = client.project_group_remove_calls.lock().unwrap();
        assert_eq!(
            calls[0],
            ProjectGroupRemoveCall {
                workspace: "ws".to_string(),
                project_key: "TEAM".to_string(),
                group_slug: "platform-eng".to_string(),
            }
        );
        assert!(client.project_group_set_calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn group_edits_fail_independently_of_each_other_and_of_direct_edits_in_the_same_batch() {
        let client = FakeClient::new()
            .with_group_set_result("locked-group", Err(ClientError::Other("boom".to_string())))
            .with_project_group_remove_result("acct-group", Err(ClientError::Unauthorized));
        let edits = vec![
            set_level_group_edit("repo-a", "locked-group", Permission::Write),
            remove_project_group_edit("TEAM", "acct-group"),
            set_level_edit("repo-b", "acct-ok", Permission::Read),
        ];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].outcome, Err(ApplyError::Other("boom".to_string())));
        assert_eq!(results[1].outcome, Err(ApplyError::Unauthorized));
        assert_eq!(results[2].outcome, Ok(()));
        assert_eq!(client.group_set_calls.lock().unwrap().len(), 1);
        assert_eq!(client.project_group_remove_calls.lock().unwrap().len(), 1);
        assert_eq!(client.set_calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn unauthorized_and_rate_limited_client_errors_surface_distinctly_for_group_edits() {
        let client = FakeClient::new()
            .with_group_remove_result("locked-group", Err(ClientError::Unauthorized))
            .with_project_group_set_result("rate-limited-group", Err(ClientError::RateLimited));
        let edits = vec![
            remove_group_edit("repo-a", "locked-group"),
            set_level_project_group_edit("TEAM", "rate-limited-group", Permission::Read),
        ];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results[0].outcome, Err(ApplyError::Unauthorized));
        assert_eq!(results[1].outcome, Err(ApplyError::RateLimited));
    }

    #[tokio::test]
    async fn a_batch_mixing_direct_and_group_edits_across_both_scopes_applies_each_independently() {
        let client = FakeClient::new();
        let edits = vec![
            set_level_edit("repo-a", "acct-1", Permission::Write),
            set_level_group_edit("repo-a", "group-1", Permission::Admin),
            remove_project_edit("TEAM", "acct-2"),
            remove_project_group_edit("TEAM", "group-2"),
        ];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results.len(), 4);
        assert!(results.iter().all(|r| r.outcome == Ok(())));
        assert_eq!(client.set_calls.lock().unwrap().len(), 1);
        assert_eq!(client.group_set_calls.lock().unwrap().len(), 1);
        assert_eq!(client.project_remove_calls.lock().unwrap().len(), 1);
        assert_eq!(client.project_group_remove_calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_project_scoped_set_level_edit_routes_to_set_project_direct_permission_with_the_right_arguments() {
        let client = FakeClient::new();
        let edits = vec![set_level_project_edit("TEAM", "acct-1", Permission::Admin)];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].outcome, Ok(()));
        let calls = client.project_set_calls.lock().unwrap();
        assert_eq!(
            calls[0],
            ProjectSetCall {
                workspace: "ws".to_string(),
                project_key: "TEAM".to_string(),
                account_id: "acct-1".to_string(),
                permission: Permission::Admin,
            }
        );
        assert!(client.set_calls.lock().unwrap().is_empty());
        assert!(client.project_remove_calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_project_scoped_remove_edit_routes_to_remove_project_direct_permission_with_the_right_arguments() {
        let client = FakeClient::new();
        let edits = vec![remove_project_edit("TEAM", "acct-1")];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].outcome, Ok(()));
        let calls = client.project_remove_calls.lock().unwrap();
        assert_eq!(
            calls[0],
            ProjectRemoveCall {
                workspace: "ws".to_string(),
                project_key: "TEAM".to_string(),
                account_id: "acct-1".to_string(),
            }
        );
        assert!(client.remove_calls.lock().unwrap().is_empty());
        assert!(client.project_set_calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn project_scoped_edits_fail_independently_of_repo_scoped_ones_in_the_same_batch() {
        let client = FakeClient::new()
            .with_project_set_result("acct-fail", Err(ClientError::Other("boom".to_string())));
        let edits = vec![
            set_level_project_edit("TEAM", "acct-fail", Permission::Write),
            set_level_edit("repo-a", "acct-ok", Permission::Read),
        ];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].outcome, Err(ApplyError::Other("boom".to_string())));
        assert_eq!(results[1].outcome, Ok(()));
        assert_eq!(client.project_set_calls.lock().unwrap().len(), 1);
        assert_eq!(client.set_calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn unauthorized_and_rate_limited_client_errors_surface_distinctly_at_project_scope_too() {
        let client = FakeClient::new()
            .with_project_set_result("acct-1", Err(ClientError::Unauthorized))
            .with_project_remove_result("acct-2", Err(ClientError::RateLimited));
        let edits = vec![
            set_level_project_edit("TEAM", "acct-1", Permission::Admin),
            remove_project_edit("TEAM", "acct-2"),
        ];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results[0].outcome, Err(ApplyError::Unauthorized));
        assert_eq!(results[1].outcome, Err(ApplyError::RateLimited));
    }

    #[tokio::test]
    async fn empty_batch_returns_empty_results() {
        let client = FakeClient::new();
        let results = apply_pending_edits(&client, "ws", vec![]).await;
        assert!(results.is_empty());
    }
}
