use serde::{Deserialize, Serialize};

/// Anything that can hold a permission grant on a repository. `id` is the stable identity
/// (Bitbucket `account_id` for a user; a group's slug for a `Group` grant) — `label` is a
/// display name only and must never be used for identity or diff keying.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Principal {
    pub id: String,
    pub label: String,
}

/// How a `PermissionRecord` was granted. `Direct` and `Group` (ADR-0002: a group's own
/// grant is a first-class Principal, independent of member resolution) exist as of PD-3;
/// `Member(group_id)` (PD-4) is a resolved group member's own grant, layered on top of
/// that group's `Group` record — never collapsed into it (ADR-0001).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(tag = "type", content = "group_id")]
pub enum AccessType {
    Direct,
    Group,
    Member(String),
}

/// Permission level. Declaration order is significant: derived `Ord` gives
/// `Read < Write < CreateRepo < Admin`, matching Bitbucket's real Project hierarchy
/// (`Admin ⊃ Create ⊃ Write ⊃ Read`). `CreateRepo` is Project-scope only, but that's a
/// `GrantScope` fact, not something this type enforces (ADR-0024).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Permission {
    Read,
    Write,
    CreateRepo,
    Admin,
}

/// Where a grant lives: directly on a repo, or on the Bitbucket Project that owns it (and so
/// cascades to every repo under that Project). Orthogonal to `AccessType` (ADR-0012) — "who
/// holds the grant" and "where it lives" are independent, composable questions, not fused into
/// one enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum GrantScope {
    Repo,
    Project,
}

impl Permission {
    /// Parses Bitbucket's permission strings ("read" | "write" | "create-repo" | "admin"),
    /// case-insensitively. Returns `None` for anything else — callers decide how to recover
    /// (never a panic here; ADR-0024).
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "read" => Some(Permission::Read),
            "write" => Some(Permission::Write),
            "create-repo" => Some(Permission::CreateRepo),
            "admin" => Some(Permission::Admin),
            _ => None,
        }
    }

    /// Collapses `CreateRepo` onto `Admin`; every other variant passes through unchanged.
    /// Used only where the code *computes* (diff classification, roster counts) — never for
    /// anything that displays a permission value (ADR-0024).
    pub fn admin_light(self) -> Permission {
        match self {
            Permission::CreateRepo => Permission::Admin,
            other => other,
        }
    }

    /// The reverse of `parse` — Bitbucket's own lowercase/hyphenated wire values, used to build
    /// the JSON body of a permissions-config write. `apply.rs` never sends `CreateRepo` in
    /// practice (ADR-0021 keeps the level picker to Read/Write/Admin), but the mapping stays
    /// total rather than partial so this function can't panic on a value `parse` accepts.
    pub fn as_str(self) -> &'static str {
        match self {
            Permission::Read => "read",
            Permission::Write => "write",
            Permission::CreateRepo => "create-repo",
            Permission::Admin => "admin",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRecord {
    pub repo_project: String,
    pub repo: String,
    pub principal: Principal,
    pub access_type: AccessType,
    pub scope: GrantScope,
    pub permission: Permission,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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

/// Per-Project, per-snapshot fetch status (ADR-0012) — a Project's own `permissions-config`
/// calls can fail independently of any of its repos' own fetches. Absent entirely (no row at
/// all) means the Project wasn't encountered this run — a distinct condition from
/// `FetchFailed`, mirroring `RepoFetchStatus`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectFetchStatus {
    pub project_key: String,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_recognizes_create_repo_case_insensitively() {
        assert_eq!(Permission::parse("create-repo"), Some(Permission::CreateRepo));
        assert_eq!(Permission::parse("CREATE-REPO"), Some(Permission::CreateRepo));
        assert_eq!(Permission::parse("Create-Repo"), Some(Permission::CreateRepo));
    }

    #[test]
    fn parse_recognizes_read_write_admin_case_insensitively() {
        assert_eq!(Permission::parse("read"), Some(Permission::Read));
        assert_eq!(Permission::parse("WRITE"), Some(Permission::Write));
        assert_eq!(Permission::parse("Admin"), Some(Permission::Admin));
    }

    #[test]
    fn parse_returns_none_for_a_nonsense_string() {
        assert_eq!(Permission::parse("superadmin"), None);
        assert_eq!(Permission::parse(""), None);
    }

    #[test]
    fn as_str_round_trips_through_parse() {
        for p in [Permission::Read, Permission::Write, Permission::CreateRepo, Permission::Admin] {
            assert_eq!(Permission::parse(p.as_str()), Some(p));
        }
    }

    #[test]
    fn true_hierarchy_ordering_holds() {
        assert!(Permission::Read < Permission::Write);
        assert!(Permission::Write < Permission::CreateRepo);
        assert!(Permission::CreateRepo < Permission::Admin);
    }

    #[test]
    fn admin_light_collapses_create_repo_onto_admin() {
        assert_eq!(Permission::CreateRepo.admin_light(), Permission::Admin);
    }

    #[test]
    fn admin_light_passes_other_variants_through_unchanged() {
        assert_eq!(Permission::Read.admin_light(), Permission::Read);
        assert_eq!(Permission::Write.admin_light(), Permission::Write);
        assert_eq!(Permission::Admin.admin_light(), Permission::Admin);
    }
}
