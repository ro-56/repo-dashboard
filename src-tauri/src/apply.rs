//! Applies a batch of staged permission edits (PD-59/PD-60/PD-61/PD-62) to live Bitbucket. Each
//! `PendingEditRequest` is routed to `BitbucketClient::set_permission`/`remove_permission` and
//! applied independently (ADR-0022) — one item failing never blocks the rest of the batch. All
//! four `(scope, target)` combinations — Direct/Repo (PD-60), Direct/Project (PD-61), Group/Repo
//! and Group/Project (PD-62) — are wired to real client calls.

use serde::{Deserialize, Serialize};

use crate::client::{BitbucketClient, ClientError, PrincipalRef};
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

impl From<&EditTarget> for PrincipalRef {
    fn from(target: &EditTarget) -> Self {
        match target {
            EditTarget::Direct(id) => PrincipalRef::Direct(id.clone()),
            EditTarget::Group(id) => PrincipalRef::Group(id.clone()),
        }
    }
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
/// back to its staged edit without relying on array-order bookkeeping. `Deserialize` (PD-70) is
/// for `refresh_snapshot`, which takes a batch of these back in as its own input — the same
/// shape `apply_pending_edits` just handed the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    pub request: PendingEditRequest,
    pub outcome: Result<(), ApplyError>,
}

/// Routes one edit to `BitbucketClient::set_permission`/`remove_permission`. `Project` scope
/// targets `edit.repo_project` as the key, cascading to every repo it owns, rather than
/// `edit.repo` — true for both `Direct` (PD-61) and `Group` (PD-62) targets.
async fn apply_one<C: BitbucketClient>(
    client: &C,
    workspace: &str,
    edit: &PendingEditRequest,
) -> Result<(), ApplyError> {
    let key = match edit.scope {
        GrantScope::Repo => &edit.repo,
        GrantScope::Project => &edit.repo_project,
    };
    let principal = PrincipalRef::from(&edit.target);

    match &edit.action {
        EditAction::SetLevel(permission) => client
            .set_permission(workspace, edit.scope, key, principal, *permission)
            .await
            .map_err(ApplyError::from),
        EditAction::Remove => client
            .remove_permission(workspace, edit.scope, key, principal)
            .await
            .map_err(ApplyError::from),
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
    use crate::client::{PrincipalRef, RepoInfo};
    use crate::normalize::{RawGroupPermission, RawMember, RawUserPermission};
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    enum CallAction {
        Set(Permission),
        Remove,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct Call {
        workspace: String,
        scope: GrantScope,
        key: String,
        principal: PrincipalRef,
        action: CallAction,
    }

    /// Follows `collect.rs`'s `FakeBitbucketClient` pattern: canned per-`(scope, id)` results
    /// plus call tracking, so tests assert externally observable behaviour (which request fired,
    /// with what arguments, and what `ApplyResult` came back) rather than internal control flow.
    /// Keying results by `(GrantScope, id)` rather than id alone avoids a latent collision if a
    /// test reuses the same principal id at both Repo and Project scope with different expected
    /// outcomes. The read-side methods are never exercised by these tests, so they return
    /// trivial empty results just to satisfy the trait.
    #[derive(Default)]
    struct FakeClient {
        results: HashMap<(GrantScope, String), Result<(), ClientError>>,
        calls: Mutex<Vec<Call>>,
    }

    impl FakeClient {
        fn new() -> Self {
            Self::default()
        }

        fn with_result(mut self, scope: GrantScope, id: &str, result: Result<(), ClientError>) -> Self {
            self.results.insert((scope, id.to_string()), result);
            self
        }
    }

    fn principal_id(principal: &PrincipalRef) -> &str {
        match principal {
            PrincipalRef::Direct(id) => id,
            PrincipalRef::Group(id) => id,
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

        async fn set_permission(
            &self,
            workspace: &str,
            scope: GrantScope,
            key: &str,
            principal: PrincipalRef,
            permission: Permission,
        ) -> Result<(), ClientError> {
            let id = principal_id(&principal).to_string();
            self.calls.lock().unwrap().push(Call {
                workspace: workspace.to_string(),
                scope,
                key: key.to_string(),
                principal,
                action: CallAction::Set(permission),
            });
            self.results.get(&(scope, id)).cloned().unwrap_or(Ok(()))
        }

        async fn remove_permission(
            &self,
            workspace: &str,
            scope: GrantScope,
            key: &str,
            principal: PrincipalRef,
        ) -> Result<(), ClientError> {
            let id = principal_id(&principal).to_string();
            self.calls.lock().unwrap().push(Call {
                workspace: workspace.to_string(),
                scope,
                key: key.to_string(),
                principal,
                action: CallAction::Remove,
            });
            self.results.get(&(scope, id)).cloned().unwrap_or(Ok(()))
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
    async fn a_set_level_edit_routes_to_set_permission_with_the_right_arguments() {
        let client = FakeClient::new();
        let edits = vec![set_level_edit("repo-a", "acct-1", Permission::Admin)];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].outcome, Ok(()));
        let calls = client.calls.lock().unwrap();
        assert_eq!(
            calls[0],
            Call {
                workspace: "ws".to_string(),
                scope: GrantScope::Repo,
                key: "repo-a".to_string(),
                principal: PrincipalRef::Direct("acct-1".to_string()),
                action: CallAction::Set(Permission::Admin),
            }
        );
    }

    #[tokio::test]
    async fn a_remove_edit_routes_to_remove_permission_with_the_right_arguments() {
        let client = FakeClient::new();
        let edits = vec![remove_edit("repo-a", "acct-1")];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].outcome, Ok(()));
        let calls = client.calls.lock().unwrap();
        assert_eq!(
            calls[0],
            Call {
                workspace: "ws".to_string(),
                scope: GrantScope::Repo,
                key: "repo-a".to_string(),
                principal: PrincipalRef::Direct("acct-1".to_string()),
                action: CallAction::Remove,
            }
        );
    }

    #[tokio::test]
    async fn a_batch_with_one_failing_item_still_applies_and_reports_the_rest() {
        let client =
            FakeClient::new().with_result(GrantScope::Repo, "acct-fail", Err(ClientError::Other("boom".to_string())));
        let edits = vec![
            set_level_edit("repo-a", "acct-fail", Permission::Write),
            set_level_edit("repo-b", "acct-ok", Permission::Read),
        ];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].outcome, Err(ApplyError::Other("boom".to_string())));
        assert_eq!(results[1].outcome, Ok(()));
        // Both calls still happened — the first failing did not block the second.
        assert_eq!(client.calls.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn unauthorized_and_rate_limited_client_errors_surface_as_distinct_typed_results() {
        let client = FakeClient::new()
            .with_result(GrantScope::Repo, "acct-1", Err(ClientError::Unauthorized))
            .with_result(GrantScope::Repo, "acct-2", Err(ClientError::RateLimited));
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
        let client = FakeClient::new().with_result(GrantScope::Repo, "acct-1", Err(ClientError::Unauthorized));
        let edits = vec![
            set_level_edit("repo-a", "acct-1", Permission::Admin),
            set_level_edit("repo-b", "acct-2", Permission::Read),
        ];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results[0].outcome, Err(ApplyError::Unauthorized));
        assert_eq!(results[1].outcome, Ok(()));
    }

    #[tokio::test]
    async fn a_group_set_level_edit_routes_to_set_permission_with_the_right_arguments() {
        let client = FakeClient::new();
        let edits = vec![set_level_group_edit("repo-a", "platform-eng", Permission::Write)];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].outcome, Ok(()));
        let calls = client.calls.lock().unwrap();
        assert_eq!(
            calls[0],
            Call {
                workspace: "ws".to_string(),
                scope: GrantScope::Repo,
                key: "repo-a".to_string(),
                principal: PrincipalRef::Group("platform-eng".to_string()),
                action: CallAction::Set(Permission::Write),
            }
        );
    }

    #[tokio::test]
    async fn a_group_remove_edit_routes_to_remove_permission_with_the_right_arguments() {
        let client = FakeClient::new();
        let edits = vec![remove_group_edit("repo-a", "platform-eng")];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].outcome, Ok(()));
        let calls = client.calls.lock().unwrap();
        assert_eq!(
            calls[0],
            Call {
                workspace: "ws".to_string(),
                scope: GrantScope::Repo,
                key: "repo-a".to_string(),
                principal: PrincipalRef::Group("platform-eng".to_string()),
                action: CallAction::Remove,
            }
        );
    }

    #[tokio::test]
    async fn a_project_scoped_group_set_level_edit_routes_to_set_permission_with_the_right_arguments() {
        let client = FakeClient::new();
        let edits = vec![set_level_project_group_edit("TEAM", "platform-eng", Permission::Admin)];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].outcome, Ok(()));
        let calls = client.calls.lock().unwrap();
        assert_eq!(
            calls[0],
            Call {
                workspace: "ws".to_string(),
                scope: GrantScope::Project,
                key: "TEAM".to_string(),
                principal: PrincipalRef::Group("platform-eng".to_string()),
                action: CallAction::Set(Permission::Admin),
            }
        );
    }

    #[tokio::test]
    async fn a_project_scoped_group_remove_edit_routes_to_remove_permission_with_the_right_arguments() {
        let client = FakeClient::new();
        let edits = vec![remove_project_group_edit("TEAM", "platform-eng")];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].outcome, Ok(()));
        let calls = client.calls.lock().unwrap();
        assert_eq!(
            calls[0],
            Call {
                workspace: "ws".to_string(),
                scope: GrantScope::Project,
                key: "TEAM".to_string(),
                principal: PrincipalRef::Group("platform-eng".to_string()),
                action: CallAction::Remove,
            }
        );
    }

    #[tokio::test]
    async fn group_edits_fail_independently_of_each_other_and_of_direct_edits_in_the_same_batch() {
        let client = FakeClient::new()
            .with_result(GrantScope::Repo, "locked-group", Err(ClientError::Other("boom".to_string())))
            .with_result(GrantScope::Project, "acct-group", Err(ClientError::Unauthorized));
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
        assert_eq!(client.calls.lock().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn unauthorized_and_rate_limited_client_errors_surface_distinctly_for_group_edits() {
        let client = FakeClient::new()
            .with_result(GrantScope::Repo, "locked-group", Err(ClientError::Unauthorized))
            .with_result(GrantScope::Project, "rate-limited-group", Err(ClientError::RateLimited));
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
        assert_eq!(client.calls.lock().unwrap().len(), 4);
    }

    #[tokio::test]
    async fn a_project_scoped_set_level_edit_routes_to_set_permission_with_the_right_arguments() {
        let client = FakeClient::new();
        let edits = vec![set_level_project_edit("TEAM", "acct-1", Permission::Admin)];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].outcome, Ok(()));
        let calls = client.calls.lock().unwrap();
        assert_eq!(
            calls[0],
            Call {
                workspace: "ws".to_string(),
                scope: GrantScope::Project,
                key: "TEAM".to_string(),
                principal: PrincipalRef::Direct("acct-1".to_string()),
                action: CallAction::Set(Permission::Admin),
            }
        );
    }

    #[tokio::test]
    async fn a_project_scoped_remove_edit_routes_to_remove_permission_with_the_right_arguments() {
        let client = FakeClient::new();
        let edits = vec![remove_project_edit("TEAM", "acct-1")];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].outcome, Ok(()));
        let calls = client.calls.lock().unwrap();
        assert_eq!(
            calls[0],
            Call {
                workspace: "ws".to_string(),
                scope: GrantScope::Project,
                key: "TEAM".to_string(),
                principal: PrincipalRef::Direct("acct-1".to_string()),
                action: CallAction::Remove,
            }
        );
    }

    #[tokio::test]
    async fn project_scoped_edits_fail_independently_of_repo_scoped_ones_in_the_same_batch() {
        let client = FakeClient::new()
            .with_result(GrantScope::Project, "acct-fail", Err(ClientError::Other("boom".to_string())));
        let edits = vec![
            set_level_project_edit("TEAM", "acct-fail", Permission::Write),
            set_level_edit("repo-a", "acct-ok", Permission::Read),
        ];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].outcome, Err(ApplyError::Other("boom".to_string())));
        assert_eq!(results[1].outcome, Ok(()));
        assert_eq!(client.calls.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn unauthorized_and_rate_limited_client_errors_surface_distinctly_at_project_scope_too() {
        let client = FakeClient::new()
            .with_result(GrantScope::Project, "acct-1", Err(ClientError::Unauthorized))
            .with_result(GrantScope::Project, "acct-2", Err(ClientError::RateLimited));
        let edits = vec![
            set_level_project_edit("TEAM", "acct-1", Permission::Admin),
            remove_project_edit("TEAM", "acct-2"),
        ];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results[0].outcome, Err(ApplyError::Unauthorized));
        assert_eq!(results[1].outcome, Err(ApplyError::RateLimited));
    }

    #[tokio::test]
    async fn results_keyed_by_scope_avoid_cross_scope_collisions_for_the_same_principal_id() {
        // Same id ("acct-1"), different scopes, different expected outcomes — proves the fake's
        // (GrantScope, id) keying (mirroring the ticket's stated risk) doesn't let a Project-scope
        // canned result leak into a Repo-scope call for the same id, or vice versa.
        let client = FakeClient::new()
            .with_result(GrantScope::Repo, "acct-1", Err(ClientError::Unauthorized))
            .with_result(GrantScope::Project, "acct-1", Ok(()));
        let edits = vec![
            set_level_edit("repo-a", "acct-1", Permission::Admin),
            set_level_project_edit("TEAM", "acct-1", Permission::Admin),
        ];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert_eq!(results[0].outcome, Err(ApplyError::Unauthorized));
        assert_eq!(results[1].outcome, Ok(()));
    }

    #[tokio::test]
    async fn empty_batch_returns_empty_results() {
        let client = FakeClient::new();
        let results = apply_pending_edits(&client, "ws", vec![]).await;
        assert!(results.is_empty());
    }
}
