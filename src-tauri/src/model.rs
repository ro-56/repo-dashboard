/// Anything that can hold a permission grant on a repository. `id` is the stable identity
/// (Bitbucket `account_id` for a user; a group's slug for a `Group` grant) — `label` is a
/// display name only and must never be used for identity or diff keying.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Principal {
    pub id: String,
    pub label: String,
}

/// How a `PermissionRecord` was granted. `Direct` and `Group` (ADR-0002: a group's own
/// grant is a first-class Principal, independent of member resolution) exist as of PD-3;
/// `Member(group_id)` (PD-4) is a resolved group member's own grant, layered on top of
/// that group's `Group` record — never collapsed into it (ADR-0001).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AccessType {
    Direct,
    Group,
    Member(String),
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

/// Per-group, per-repo, per-snapshot record of whether that group's membership list was
/// resolvable. `true` for both a successful-but-empty fetch and a successful non-empty
/// fetch; `false` only when the fetch itself failed or was inaccessible. Not consumed by
/// the diff engine in v1 (PD-1 user stories 14/17) — retained for future visibility-flap
/// detection, kept distinct from "confirmed empty".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupMembershipStatus {
    pub repo_project: String,
    pub repo: String,
    pub group_id: String,
    pub members_resolved: bool,
}
