//! Abstraction over the Bitbucket Cloud HTTP API. `BitbucketClient` hides pagination and
//! retry policy behind a small trait so `collect_and_store` (`collect.rs`) can be tested
//! against canned responses, with `RealBitbucketClient` the only implementation that ever
//! makes a real network call.

use crate::model::{GrantScope, Permission};
use crate::normalize::{RawGroupMembersResponse, RawGroupPermission, RawMember, RawUserPermission};

/// Which kind of Principal a permission write targets — a Bitbucket `account_id` (Direct) or a
/// group slug (Group). Owned here rather than reusing `apply.rs`'s `EditTarget` so `client.rs`
/// doesn't depend on the apply-layer type; `apply_one` converts `EditTarget` into this at the
/// call site.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PrincipalRef {
    Direct(String),
    Group(String),
}

/// Distinguishes the failure classes `collect_and_store` needs to treat differently:
/// `Unauthorized` aborts the whole Run, `RateLimited` is retried once by the client itself
/// before surfacing, and `Other` is any other transient/unexpected failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientError {
    Unauthorized,
    RateLimited,
    Other(String),
}

/// One repository from the workspace repo-list call, with its project captured from the
/// same response (PRD 5.2 — no extra endpoint needed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoInfo {
    pub repo_project: String,
    pub repo: String,
}

/// A repo/group/member listing call, each following Bitbucket's `next`-cursor pagination
/// transparently so callers always receive a fully materialized list.
pub trait BitbucketClient {
    fn list_repositories(
        &self,
        workspace: &str,
    ) -> impl std::future::Future<Output = Result<Vec<RepoInfo>, ClientError>> + Send;

    fn list_direct_permissions(
        &self,
        workspace: &str,
        repo: &str,
    ) -> impl std::future::Future<Output = Result<Vec<RawUserPermission>, ClientError>> + Send;

    fn list_group_permissions(
        &self,
        workspace: &str,
        repo: &str,
    ) -> impl std::future::Future<Output = Result<Vec<RawGroupPermission>, ClientError>> + Send;

    fn list_group_members(
        &self,
        workspace: &str,
        group_slug: &str,
    ) -> impl std::future::Future<Output = Result<Vec<RawMember>, ClientError>> + Send;

    fn list_project_direct_permissions(
        &self,
        workspace: &str,
        project_key: &str,
    ) -> impl std::future::Future<Output = Result<Vec<RawUserPermission>, ClientError>> + Send;

    fn list_project_group_permissions(
        &self,
        workspace: &str,
        project_key: &str,
    ) -> impl std::future::Future<Output = Result<Vec<RawGroupPermission>, ClientError>> + Send;

    /// Sets a grant's level on a repo or Project, for either a Direct or Group principal
    /// — `key` is the repo slug when `scope == GrantScope::Repo`, the
    /// project key when `scope == GrantScope::Project`. `PUT
    /// .../permissions-config/{users,groups}/{id}` — requires `repository:admin` (Repo scope) or
    /// `project:admin` (Project scope), checked only lazily (ADR-0023).
    fn set_permission(
        &self,
        workspace: &str,
        scope: GrantScope,
        key: &str,
        principal: PrincipalRef,
        permission: Permission,
    ) -> impl std::future::Future<Output = Result<(), ClientError>> + Send;

    /// Removes a grant from a repo or Project entirely, for either a Direct or Group principal.
    /// `DELETE` on the same `permissions-config/{users,groups}/{id}` path as `set_permission`.
    fn remove_permission(
        &self,
        workspace: &str,
        scope: GrantScope,
        key: &str,
        principal: PrincipalRef,
    ) -> impl std::future::Future<Output = Result<(), ClientError>> + Send;
}

const RETRY_BACKOFF: std::time::Duration = std::time::Duration::from_millis(500);

pub struct RealBitbucketClient {
    http: reqwest::Client,
    username: String,
    app_password: String,
}

impl RealBitbucketClient {
    pub fn new(username: impl Into<String>, app_password: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            username: username.into(),
            app_password: app_password.into(),
        }
    }

    async fn get_once(&self, url: &str) -> Result<serde_json::Value, ClientError> {
        let resp = self
            .http
            .get(url)
            .basic_auth(&self.username, Some(&self.app_password))
            .send()
            .await
            .map_err(|e| ClientError::Other(e.to_string()))?;

        match resp.status().as_u16() {
            200 => resp
                .json::<serde_json::Value>()
                .await
                .map_err(|e| ClientError::Other(e.to_string())),
            401 => Err(ClientError::Unauthorized),
            429 => Err(ClientError::RateLimited),
            other => Err(ClientError::Other(format!("unexpected status {other}"))),
        }
    }

    /// A single retry, after a short fixed backoff, when the first attempt is `RateLimited`.
    /// `Unauthorized` is never retried.
    async fn get_with_retry(&self, url: &str) -> Result<serde_json::Value, ClientError> {
        match self.get_once(url).await {
            Err(ClientError::RateLimited) => {
                tokio::time::sleep(RETRY_BACKOFF).await;
                self.get_once(url).await
            }
            result => result,
        }
    }

    async fn write_once(
        &self,
        method: reqwest::Method,
        url: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<(), ClientError> {
        let mut req = self.http.request(method, url).basic_auth(&self.username, Some(&self.app_password));
        if let Some(body) = body {
            req = req.json(body);
        }
        let resp = req.send().await.map_err(|e| ClientError::Other(e.to_string()))?;

        match resp.status().as_u16() {
            200..=204 => Ok(()),
            401 => Err(ClientError::Unauthorized),
            429 => Err(ClientError::RateLimited),
            other => Err(ClientError::Other(format!("unexpected status {other}"))),
        }
    }

    /// A single retry, after a short fixed backoff, when the first attempt is `RateLimited` —
    /// mirrors `get_with_retry`'s policy for the write side.
    async fn write_with_retry(
        &self,
        method: reqwest::Method,
        url: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<(), ClientError> {
        match self.write_once(method.clone(), url, body).await {
            Err(ClientError::RateLimited) => {
                tokio::time::sleep(RETRY_BACKOFF).await;
                self.write_once(method, url, body).await
            }
            result => result,
        }
    }

    /// Builds the `permissions-config/{users,groups}/{id}` URL for a write, branching once on
    /// `scope` for the base path and once on `principal` for the segment — shared by
    /// `set_permission` and `remove_permission`.
    fn permission_url(&self, workspace: &str, scope: GrantScope, key: &str, principal: &PrincipalRef) -> String {
        let base = match scope {
            GrantScope::Repo => format!("https://api.bitbucket.org/2.0/repositories/{workspace}/{key}"),
            GrantScope::Project => {
                format!("https://api.bitbucket.org/2.0/workspaces/{workspace}/projects/{key}")
            }
        };
        match principal {
            PrincipalRef::Direct(account_id) => {
                format!("{base}/permissions-config/users/{account_id}")
            }
            PrincipalRef::Group(group_slug) => {
                format!("{base}/permissions-config/groups/{group_slug}")
            }
        }
    }

    async fn get_paginated(&self, url: String) -> Result<Vec<serde_json::Value>, ClientError> {
        let mut results = Vec::new();
        let mut next = Some(url);
        while let Some(page_url) = next {
            let page = self.get_with_retry(&page_url).await?;
            let values = page
                .get("values")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            results.extend(values);
            next = page
                .get("next")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
        }
        Ok(results)
    }
}

impl BitbucketClient for RealBitbucketClient {
    async fn list_repositories(&self, workspace: &str) -> Result<Vec<RepoInfo>, ClientError> {
        let url = format!("https://api.bitbucket.org/2.0/repositories/{workspace}");
        let values = self.get_paginated(url).await?;
        Ok(values
            .into_iter()
            .filter_map(|v| {
                let repo = v.get("slug")?.as_str()?.to_string();
                let repo_project = v.get("project")?.get("key")?.as_str()?.to_string();
                Some(RepoInfo { repo_project, repo })
            })
            .collect())
    }

    async fn list_direct_permissions(
        &self,
        workspace: &str,
        repo: &str,
    ) -> Result<Vec<RawUserPermission>, ClientError> {
        let url = format!(
            "https://api.bitbucket.org/2.0/repositories/{workspace}/{repo}/permissions-config/users"
        );
        let values = self.get_paginated(url).await?;
        Ok(values
            .into_iter()
            .filter_map(|v| {
                let user = v.get("user")?;
                Some(RawUserPermission {
                    account_id: user.get("account_id")?.as_str()?.to_string(),
                    display_name: user.get("display_name")?.as_str()?.to_string(),
                    permission: v.get("permission")?.as_str()?.to_string(),
                })
            })
            .collect())
    }

    async fn list_group_permissions(
        &self,
        workspace: &str,
        repo: &str,
    ) -> Result<Vec<RawGroupPermission>, ClientError> {
        let url = format!(
            "https://api.bitbucket.org/2.0/repositories/{workspace}/{repo}/permissions-config/groups"
        );
        let values = self.get_paginated(url).await?;
        Ok(values
            .into_iter()
            .filter_map(|v| {
                let group = v.get("group")?;
                Some(RawGroupPermission {
                    group_slug: group.get("slug")?.as_str()?.to_string(),
                    group_name: group.get("name")?.as_str()?.to_string(),
                    permission: v.get("permission")?.as_str()?.to_string(),
                    // Placeholder — `collect_and_store` fetches each group's members
                    // separately (once per Run, cached by group id) and overwrites this field
                    // before normalization, so the value set here is never actually consumed.
                    members: RawGroupMembersResponse::FetchFailed,
                })
            })
            .collect())
    }

    async fn list_group_members(
        &self,
        workspace: &str,
        group_slug: &str,
    ) -> Result<Vec<RawMember>, ClientError> {
        let url = format!(
            "https://api.bitbucket.org/2.0/workspaces/{workspace}/permissions/config/groups/{group_slug}/members"
        );
        let values = self.get_paginated(url).await?;
        Ok(values
            .into_iter()
            .filter_map(|v| {
                Some(RawMember {
                    account_id: v.get("account_id")?.as_str()?.to_string(),
                    display_name: v.get("display_name")?.as_str()?.to_string(),
                })
            })
            .collect())
    }

    async fn list_project_direct_permissions(
        &self,
        workspace: &str,
        project_key: &str,
    ) -> Result<Vec<RawUserPermission>, ClientError> {
        let url = format!(
            "https://api.bitbucket.org/2.0/workspaces/{workspace}/projects/{project_key}/permissions-config/users"
        );
        let values = self.get_paginated(url).await?;
        Ok(values
            .into_iter()
            .filter_map(|v| {
                let user = v.get("user")?;
                Some(RawUserPermission {
                    account_id: user.get("account_id")?.as_str()?.to_string(),
                    display_name: user.get("display_name")?.as_str()?.to_string(),
                    permission: v.get("permission")?.as_str()?.to_string(),
                })
            })
            .collect())
    }

    async fn list_project_group_permissions(
        &self,
        workspace: &str,
        project_key: &str,
    ) -> Result<Vec<RawGroupPermission>, ClientError> {
        let url = format!(
            "https://api.bitbucket.org/2.0/workspaces/{workspace}/projects/{project_key}/permissions-config/groups"
        );
        let values = self.get_paginated(url).await?;
        Ok(values
            .into_iter()
            .filter_map(|v| {
                let group = v.get("group")?;
                Some(RawGroupPermission {
                    group_slug: group.get("slug")?.as_str()?.to_string(),
                    group_name: group.get("name")?.as_str()?.to_string(),
                    permission: v.get("permission")?.as_str()?.to_string(),
                    // Placeholder — `collect_and_store` resolves each group's members
                    // separately (once per Run, cached by group id, shared with repo-level
                    // group resolution) and overwrites this field before normalization.
                    members: RawGroupMembersResponse::FetchFailed,
                })
            })
            .collect())
    }

    async fn set_permission(
        &self,
        workspace: &str,
        scope: GrantScope,
        key: &str,
        principal: PrincipalRef,
        permission: Permission,
    ) -> Result<(), ClientError> {
        let url = self.permission_url(workspace, scope, key, &principal);
        let body = serde_json::json!({ "permission": permission.as_str() });
        self.write_with_retry(reqwest::Method::PUT, &url, Some(&body)).await
    }

    async fn remove_permission(
        &self,
        workspace: &str,
        scope: GrantScope,
        key: &str,
        principal: PrincipalRef,
    ) -> Result<(), ClientError> {
        let url = self.permission_url(workspace, scope, key, &principal);
        self.write_with_retry(reqwest::Method::DELETE, &url, None).await
    }
}
