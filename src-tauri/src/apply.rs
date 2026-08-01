//! Applies a batch of staged permission edits (PD-59/PD-60) to live Bitbucket. Each
//! `PendingEditRequest` is routed to the matching `BitbucketClient` write method and applied
//! independently (ADR-0022) — one item failing never blocks the rest of the batch. Only the
//! Direct/Repo combination is wired to a real client call in this ticket; Group and Project
//! targets exist on `EditTarget`/`GrantScope` so Tickets 2/3 can reuse this same shape without
//! remodeling it, but routing them here is out of scope until the UI can produce them.

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

/// Routes one edit to the matching `BitbucketClient` write method. Only `Repo` + `Direct` is
/// wired for real in this ticket (PD-60) — every other scope/target combination isn't
/// reachable from the UI yet (menus only render for Direct/Repo entries), so it surfaces as an
/// `Other` error rather than panicking, keeping this match total.
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
        _ => Err(ApplyError::Other(
            "this scope/target combination isn't supported yet".to_string(),
        )),
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

    /// Follows `collect.rs`'s `FakeBitbucketClient` pattern: canned per-key results plus call
    /// tracking, so tests assert externally observable behaviour (which method was called, with
    /// what arguments, and what `ApplyResult` came back) rather than internal control flow. The
    /// read-side methods are never exercised by these tests, so they return trivial empty
    /// results just to satisfy the trait.
    #[derive(Default)]
    struct FakeClient {
        set_results: HashMap<String, Result<(), ClientError>>,
        remove_results: HashMap<String, Result<(), ClientError>>,
        set_calls: Mutex<Vec<SetCall>>,
        remove_calls: Mutex<Vec<RemoveCall>>,
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
    async fn a_group_target_is_not_yet_routed_and_reports_a_typed_error_rather_than_panicking() {
        let client = FakeClient::new();
        let edits = vec![PendingEditRequest {
            scope: GrantScope::Repo,
            target: EditTarget::Group("platform-eng".to_string()),
            repo_project: "TEAM".to_string(),
            repo: "repo-a".to_string(),
            action: EditAction::SetLevel(Permission::Write),
        }];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert!(matches!(results[0].outcome, Err(ApplyError::Other(_))));
        assert!(client.set_calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_project_scoped_direct_edit_is_not_yet_routed_and_reports_a_typed_error() {
        let client = FakeClient::new();
        let edits = vec![PendingEditRequest {
            scope: GrantScope::Project,
            target: EditTarget::Direct("acct-1".to_string()),
            repo_project: "TEAM".to_string(),
            repo: "repo-a".to_string(),
            action: EditAction::Remove,
        }];

        let results = apply_pending_edits(&client, "ws", edits).await;

        assert!(matches!(results[0].outcome, Err(ApplyError::Other(_))));
        assert!(client.remove_calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn empty_batch_returns_empty_results() {
        let client = FakeClient::new();
        let results = apply_pending_edits(&client, "ws", vec![]).await;
        assert!(results.is_empty());
    }
}
