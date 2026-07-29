# repo-dashboard

Domain glossary for repo-dashboard, a local audit tool for Bitbucket Cloud repository access.

## Language

**Principal**:
Anything that can hold a permission grant on a repository — a user or a group. A group is not just metadata attached to its members; it is itself grantable and diffable, independent of whether its membership is known. A user Principal's identity is Bitbucket's stable `account_id`/`uuid`, never the user-editable nickname — nickname and display name are labels, not identity, so a nickname change between two Runs must never look like a Revoke plus a Grant.
_Avoid_: Grantee, subject

**Direct grant**:
A permission held by a user Principal, assigned to them individually rather than through a group.
_Avoid_: Individual permission

**Group grant**:
A permission held by a group Principal itself. Recorded whenever the group's own permission is fetched successfully, regardless of whether the group's membership can be resolved — "this group has admin access" is meaningful on its own.
_Avoid_: Group permission (ambiguous with a member's group-derived permission)

**Member grant**:
A permission a user Principal holds because they belong to a group that holds a Group grant. Only recorded when the group's membership is resolvable. A user can simultaneously hold a Direct grant and one or more Member grants on the same repo — each is tracked as its own record, never collapsed into one.
_Avoid_: Inherited permission, indirect permission

**Unresolvable membership**:
The state where a group's own grant was fetched successfully but its member list could not be — typically because the credential lacks the workspace-level scope required to list group members. Distinct from an empty group (member list fetched successfully, zero members). Only the Group grant is recorded in this state; no Member grants are derived for that group in that Snapshot.
_Avoid_: Empty group, inaccessible group

**Fetch failure**:
A repo that was discovered, but whose permissions could not be retrieved this Run (e.g. the permissions-config call errored). Distinct from a repo absent from a Run (never discovered at all) and from a repo with zero grants (fetched successfully, empty result). Tracked as a per-repo status, not as a permission record, since there is no Principal to attach it to. Recoverable at the granularity of a single repo — the rest of the Run continues.
_Avoid_: Inaccessible repo, "no access" (main.py's SEM ACESSO OU INACESSÍVEL conflated this with "zero grants")

**Discovery failure**:
The top-level list-repositories call for a Run failing outright (e.g. the credential itself is rejected with a 401) — as opposed to a Fetch failure, which is scoped to one already-discovered repo. A Discovery failure means there is no repo set to attach a Snapshot to, so it blocks the Run entirely: no Snapshot is saved, and the failure is surfaced to the user directly rather than appearing as a pile of per-repo statuses.
_Avoid_: Fetch failure (that term is reserved for a single repo's permissions call failing after discovery succeeded), Run failure (too generic — a Run with several Fetch failures is not a failed Run, it's a partial Snapshot)

**Run**:
The user-facing action of triggering a new data collection pass ("Run now"). Produces exactly one Snapshot.
_Avoid_: Snapshot (the noun for the stored result, not the action of producing it)

**Snapshot**:
One immutable, timestamped Run's worth of collected data across the whole workspace. Never overwritten or edited after the fact.

**Grant** / **Revoke**:
Diffed per individual permission record (a specific Principal + repo + grant source — Direct, Group, or Member). A Grant is a record present in the later Snapshot with no matching record in the earlier one; a Revoke is the reverse. A user gaining a new, higher-level Member grant while keeping an unrelated lower-level Direct grant is a Grant, not a Level change — the two records are never merged into one "effective" figure in v1.
_Avoid_: Add/remove, gained/lost access

**Level change**:
The same permission record (same Principal, repo, and grant source) present in both compared Snapshots with a different permission. An **Escalation** if it moved up (`read < write < admin`), a **Demotion** if it moved down.
_Avoid_: Change (too generic — Grant/Revoke are also "changes")
