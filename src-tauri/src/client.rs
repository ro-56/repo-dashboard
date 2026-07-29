//! Abstraction over the Bitbucket Cloud HTTP API. `BitbucketClient` hides pagination and
//! retry policy behind a small trait so `collect_and_store` (`collect.rs`) can be tested
//! against canned responses, with `RealBitbucketClient` the only implementation that ever
//! makes a real network call.

use crate::normalize::{RawGroupMembersResponse, RawGroupPermission, RawUserPermission};

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
    /// `Unauthorized` is never retried, per PD-7's acceptance criteria.
    async fn get_with_retry(&self, url: &str) -> Result<serde_json::Value, ClientError> {
        match self.get_once(url).await {
            Err(ClientError::RateLimited) => {
                tokio::time::sleep(RETRY_BACKOFF).await;
                self.get_once(url).await
            }
            result => result,
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
                    // Group-membership fetching is PD-9's scope; every group is reported as
                    // unresolved until then rather than resolved-and-empty.
                    members: RawGroupMembersResponse::FetchFailed,
                })
            })
            .collect())
    }
}
