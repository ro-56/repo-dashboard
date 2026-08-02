# repo-dashboard

Domain glossary for repo-dashboard, a local audit tool for Bitbucket Cloud repository access.

## Language

**Principal**:
Anything that can hold a permission grant on a repository — a user or a group. A group is not just metadata attached to its members; it is itself grantable and diffable, independent of whether its membership is known. A user Principal's identity is Bitbucket's stable `account_id`/`uuid`, never the user-editable nickname — nickname and display name are labels, not identity, so a nickname change between two Runs must never look like a Revoke plus a Grant.
_Avoid_: Grantee, subject

**Direct grant**:
A permission held by a user Principal, assigned to them individually rather than through a group. Exists at either Grant scope (Repo-level or Project-level).
_Avoid_: Individual permission

**Group grant**:
A permission held by a group Principal itself. Recorded whenever the group's own permission is fetched successfully, regardless of whether the group's membership can be resolved — "this group has admin access" is meaningful on its own. Exists at either Grant scope (Repo-level or Project-level).
_Avoid_: Group permission (ambiguous with a member's group-derived permission)

**Member grant**:
A permission a user Principal holds because they belong to a group that holds a Group grant. Only recorded when the group's membership is resolvable. A user can simultaneously hold a Direct grant and one or more Member grants on the same repo — each is tracked as its own record, never collapsed into one. Exists at either Grant scope (Repo-level or Project-level), following whichever scope the underlying Group grant was fetched at. Not independently editable — a Member grant has no Bitbucket resource of its own to change; only the underlying Group grant can be edited or removed (ADR-0021, `docs/adr/0021-permission-editing-scope-boundaries.md`).
_Avoid_: Inherited permission, indirect permission

**Create-repo permission**:
A fourth Permission level, Project-scope only, sitting between Write and Admin in Bitbucket's real hierarchy (`Read < Write < Create-repo < Admin` — each level includes everything below it). Displayed as its own distinct value wherever a permission is shown directly, but collapsed onto Admin everywhere the code computes rather than displays — diff classification (Grant/Revoke/Escalation/Demotion) and roster summary counts treat it as admin-equivalent, so a Create-repo ↔ Admin transition between two Runs produces no diff event at all (ADR-0024, `docs/adr/0024-create-repo-modeled-distinct-computed-as-admin.md`).
_Avoid_: create-repo-as-admin (the two are distinguished on display, just not in computation), a fifth "none" permission (considered but not real — not a documented value for this API, see ADR-0024)

**Grant scope**:
Whether a grant lives at the repo itself (Repo-level) or at its Project, cascading to every repo the Project owns (Project-level). Orthogonal to Direct/Group/Member, which describe *who* holds the grant — the same three grant kinds exist at either scope, and a Principal can simultaneously hold, e.g., a Repo-level Direct grant and a Project-level Direct grant on the same repo as two independent records, never collapsed (extends ADR-0001's per-grant-source diffing to this new dimension; see ADR-0012, `docs/adr/0012-project-level-grants-orthogonal-scope.md`).
_Avoid_: Project permission (conflates the grant's scope with the Project entity itself), inherited permission (implies a weaker/derived status this tool doesn't distinguish — a Project-level grant is just as real as a Repo-level one)

**Project**:
The Bitbucket Project a repo belongs to. Previously just a grouping label surfaced in the Roster tree; now also a Grant scope in its own right — a Project can hold Direct and Group grants that cascade to every repo it owns, and its own fetch can succeed or fail independently of any one repo's fetch (see Project fetch failure).
_Avoid_: Folder, category (a Project is a first-class permission-bearing entity here, not just an organizational grouping)

**Unresolvable membership**:
The state where a group's own grant was fetched successfully but its member list could not be — typically because the credential lacks the workspace-level scope required to list group members. Distinct from an empty group (member list fetched successfully, zero members). Only the Group grant is recorded in this state; no Member grants are derived for that group in that Snapshot.
_Avoid_: Empty group, inaccessible group

**Fetch failure**:
A repo that was discovered, but whose own permissions could not be retrieved this Run (e.g. the repo-level permissions-config call errored). Distinct from a repo absent from a Run (never discovered at all), from a repo with zero grants (fetched successfully, empty result), and from a Project fetch failure (scoped to the Project, not this one repo — a repo can be fully Ok itself while its Project layer failed). Tracked as a per-repo status, not as a permission record, since there is no Principal to attach it to. Recoverable at the granularity of a single repo — the rest of the Run continues.
_Avoid_: Inaccessible repo, "no access" (conflates a failed fetch with a repo that has zero grants)

**Project fetch failure**:
A Project's own permissions-config call failing for a Run (e.g. the credential lacks Project-level scope). Tracked once per Project per Snapshot, not duplicated across its repos — the Roster tree joins a Project's status onto every repo it owns at query time, the same way a Group grant is recorded once and referenced by its resolved Member grants. Distinct from a repo-level Fetch failure (one repo's own call failing) and from a Discovery failure (the workspace-wide call failing).
_Avoid_: A per-repo flattened marker row duplicated across every affected repo (replaced here by a first-class per-Project status, not a duplicated PermissionRecord)

**Discovery failure**:
The top-level list-repositories call for a Run failing outright (e.g. the credential itself is rejected with a 401) — as opposed to a Fetch failure, which is scoped to one already-discovered repo. A Discovery failure means there is no repo set to attach a Snapshot to, so it blocks the Run entirely: no Snapshot is saved, and the failure is surfaced to the user directly rather than appearing as a pile of per-repo statuses.
_Avoid_: Fetch failure (that term is reserved for a single repo's permissions call failing after discovery succeeded), Run failure (too generic — a Run with several Fetch failures is not a failed Run, it's a partial Snapshot)

**Run**:
The user-facing action of triggering a new data collection pass ("Run now"), always via full workspace discovery (`list_repositories`) followed by a fetch of every discovered repo and Project. Produces exactly one Snapshot. Distinct from a **Refresh**, which also produces a new Snapshot but never discovers — it copies a source Snapshot forward and only re-fetches a narrow, explicitly-addressed set of repos/projects.
_Avoid_: Snapshot (the noun for the stored result, not the action of producing it)

**Snapshot**:
One immutable, timestamped Run's worth of collected data across the whole workspace. Never overwritten or edited after the fact — but "immutable" means content, not existence: a Snapshot can be deleted outright by the user (ADR-0011, `docs/adr/0011-snapshots-are-deletable.md`), it just can never be partially changed while it exists.

**Grant** / **Revoke**:
Diffed per individual permission record (a specific Principal + repo + grant source — Direct, Group, or Member). A Grant is a record present in the later Snapshot with no matching record in the earlier one; a Revoke is the reverse. A user gaining a new, higher-level Member grant while keeping an unrelated lower-level Direct grant is a Grant, not a Level change — the two records are never merged into one "effective" figure in v1.
_Avoid_: Add/remove, gained/lost access

**Level change**:
The same permission record (same Principal, repo, and grant source) present in both compared Snapshots with a different permission. An **Escalation** if it moved up (`read < write < admin`), a **Demotion** if it moved down.
_Avoid_: Change (too generic — Grant/Revoke are also "changes")

**Run pair**:
The two Snapshots selected for comparison in the dashboard, named by role: the **Baseline** (the earlier state, "what it was") and the **Comparison** (the state being audited, "what it is now"). Selecting the same Snapshot for both is a valid Run pair — it produces a pure roster view with no diff, not an error. Grant/Revoke/Level change are always expressed as movement *from* the Baseline *to* the Comparison.
_Avoid_: Snapshot A / Snapshot B (positional, says nothing about direction), "Comparison" used alone for the pair itself, run selection

**Repo absence** / **Repo arrival**:
A repo present in one Snapshot's discovery and not the other. Absence means every grant on it reads as a Revoke; arrival means every grant reads as a Grant. Both are properties of discovery, not of any Principal, and both are stated once at the repo level rather than repeated on every row underneath. Distinct from a **Fetch failure**, where the repo *was* discovered and its roster is unknown rather than empty.
_Avoid_: Deleted repo, missing repo (a repo can be absent because it was renamed, archived, or made invisible to the credential — absence is not deletion)

**Roster tree**:
The project → repo → Principal structure returned for a given Run pair, with Grant/Revoke/Level-change markers attached in place. Computed fresh from the Run pair's full PermissionRecord/RepoFetchStatus sets on every request — never materialized or cached (see ADR-0004, `docs/adr/0004-roster-tree-computed-per-request.md`).
_Avoid_: Diff result (too narrow — the tree also carries un-diffed roster data when the Run pair is a single Snapshot compared to itself)

**Pending edit**:
A staged Level-change or Remove grant against a Direct or Group grant, held only in local UI state until Apply — never partial, never sent automatically. Only possible while the Comparison is the latest Snapshot; discarded outright (not carried forward) if a new Run or Refresh produces a Snapshot that supersedes the one it was staged against (ADR-0022, `docs/adr/0022-permission-edits-staged-batch-apply.md`) — a Refresh supersedes the source Snapshot exactly as a Run does, so the same clearing rule applies to whichever staged edits are left over (e.g. ones that failed to apply).
_Avoid_: Draft, unsaved change

**Apply**:
The single global action that commits every currently staged Pending edit to Bitbucket as live grant-config `PUT`/`DELETE` calls, after a confirm step listing every staged change. Each Pending edit within the batch is applied independently — one failing (insufficient credential scope, a 404 from a grant already changed elsewhere, rate limiting) does not block the rest.
_Avoid_: Save, Commit, Sync

**Remove grant**:
The live action of deleting a Direct or Group grant via Bitbucket's permissions-config API, staged as a Pending edit before Apply. Deliberately distinct from Revoke — Revoke is a diff outcome observed between two already-collected Snapshots; Remove grant is a user-initiated mutation of Bitbucket's current state, made through this dashboard.
_Avoid_: Revoke (reserved for the diff outcome), Delete

**Refresh**:
The user-initiated complement to Apply, offered only in the Apply results view for the batch that was just applied: it creates one new Snapshot by copying every repo/project untouched by that batch verbatim from the source Snapshot (the one the batch's edits were staged against), and re-fetching live data only for the **Refresh targets** addressed by the batch's *successful* edits. Failed edits stay staged for retry (per Apply) and contribute no target. Never calls `list_repositories` — a Run is still the only way to pick up a repo newly created, deleted, or moved between Projects. Dismissing the Apply results view retires the opportunity; a full Run is the only path forward afterward. Clears every remaining staged Pending edit, same as a Run, and auto-selects the new Snapshot as Comparison — also advancing Baseline to the pre-refresh Comparison (the Snapshot the batch was staged against), so the pair isolates exactly what the batch changed (ADR-0027).
_Avoid_: Partial Run (blurs the rule that Run always means full discovery), Sync (already reserved-against for Apply — and Sync would run the wrong direction: Refresh pulls Bitbucket → local, Apply pushes local → Bitbucket)

**Refresh target**:
The one repo (for a Repo-scope edit) or Project (for a Project-scope edit) whose grants a Refresh re-fetches live, identified directly from a successful edit's `repo`/`repo_project`. A Project-scope target cascades to every repo the source Snapshot already records under that Project — not a freshly discovered repo list — since Project-scope grants are duplicated onto every owned repo's records at collection time (see Grant scope). A `FetchFailed` on a target means zero records for it, identical to a Run's convention, since a Snapshot carries no marker distinguishing whether a Run or a Refresh produced it.
_Avoid_: Affected repo (imprecise about Project-scope cascading), Dirty repo

**Display language**:
The UI language a given install renders in — auto-detected from the OS/webview locale on first launch, overridable via an explicit picker in AppearancePanel (same persistence pattern as Theme: an explicit stored choice always wins over a later OS change). Purely a rendering concern; each install is still single-user, single-workspace regardless of language. Every app-authored string, including domain vocabulary (Grant, Revoke, Escalation, Demotion, Level change, etc.), renders in the selected Display language — this glossary's canonical terms name the *concept*, not a fixed English string, and each language gets its own precise word for it. Bitbucket-sourced content (repo/Project names, Principal labels) is never translated — it's passed through verbatim regardless of Display language, since it isn't this app's content to translate. Dates always render as unambiguous ISO (`YYYY-MM-DD`), never locale-formatted. The CSV Export and backend-originated error messages are deliberate exceptions, always rendering in English regardless of Display language (ADR-0029, `docs/adr/0029-backend-errors-excluded-from-localization.md`; ADR-0030, `docs/adr/0030-csv-export-always-english.md`).
_Avoid_: Locale (this app only translates strings, it doesn't localize number/date formatting — "locale" implies more than this app does)

**Export**:
A one-off CSV file generated from the whole-workspace Roster tree for sharing with a reviewer or offline archiving — not a recurring reporting path, and the dashboard remains the source of truth regardless (ADR-0026, `docs/adr/0026-roster-export-csv-only-carve-out.md`). Always a flat, unfiltered, diff-free roster of the Comparison Snapshot alone — no diff markers, no view-mode/filter state, no notion of "what's on screen." Produced via a native Save As dialog, never sent by the app itself.
_Avoid_: Report (implies a recurring artifact), download (says nothing about format/content scope)
