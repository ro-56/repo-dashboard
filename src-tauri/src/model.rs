/// Anything that can hold a permission grant on a repository. `id` is the stable identity
/// (Bitbucket `account_id` for a user; a group's slug/id once Group grants are added) —
/// `label` is a display name only and must never be used for identity or diff keying.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Principal {
    pub id: String,
    pub label: String,
}

/// How a `PermissionRecord` was granted. Only `Direct` exists in the PD-2 tracer bullet;
/// `Group` and `Member(group_id)` are added in PD-3/PD-4 without reshaping `PermissionRecord`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AccessType {
    Direct,
}

/// Permission level. Declaration order is significant: derived `Ord` gives `Read < Write < Admin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Permission {
    Read,
    Write,
    Admin,
}

impl Permission {
    /// Parses Bitbucket's permission strings ("read" | "write" | "admin"), case-insensitively.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "read" => Some(Permission::Read),
            "write" => Some(Permission::Write),
            "admin" => Some(Permission::Admin),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRecord {
    pub repo_project: String,
    pub repo: String,
    pub principal: Principal,
    pub access_type: AccessType,
    pub permission: Permission,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoStatus {
    Ok,
    FetchFailed,
}

/// Per-repo, per-snapshot fetch status. Absent entirely (no row at all) means the repo
/// wasn't discovered this run — a distinct condition from `FetchFailed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoFetchStatus {
    pub repo_project: String,
    pub repo: String,
    pub status: RepoStatus,
}
