---
created: "2026-07-29T00:01:29Z"
dependencies:
    - PD-1
id: PD-5
links: []
mode: afk
priority: 2
status: open
tags: []
title: 'Bitbucket collection: HTTP client, credential entry, SQLite storage (Run now tracer bullet)'
type: feature
updated: "2026-07-29T00:01:37Z"
---

## Problem Statement

The Principal/PermissionRecord domain model, normalization, and diff engine (PD-1 through PD-4) are complete and fully tested, but they exist in a vacuum — there is no way to actually run them against real Bitbucket data. There's nowhere to enter a Bitbucket credential, nothing that calls the Bitbucket API, and nothing that persists a Run's results. An auditor still cannot use repo-dashboard for its stated purpose: repo-dashboard cannot yet answer "who has access right now" for a single real workspace, let alone across two runs.

## Solution

Build the "Run now" pipeline end to end, as a single tracer bullet: a minimal UI-driven credential entry, a Bitbucket HTTP client that performs dynamic repo discovery and collects direct/group/member permissions per repo, and SQLite persistence of the result as an immutable Snapshot — all wired through one Tauri command. This proves the full data flow described in CLAUDE.md (`"Run now" → Tauri command → Bitbucket fetch → normalize → persist Snapshot`) against a real workspace, before any dashboard UI is built on top of it. The existing pure `normalize`/`diff` functions from PD-1–PD-4 are consumed unmodified.

## User Stories

1. As an auditor, I want to enter my Bitbucket username, app password, and workspace through the app UI, so that I can authenticate without editing a config file or setting an environment variable.
2. As an auditor, I want to replace my stored credential from the same UI at any time, so that I can rotate it without restarting the app.
3. As an auditor, I want my credential to never be written to disk in plaintext, so that a compromised machine doesn't leak my Bitbucket password from this app alone.
4. As an auditor, I want to trigger a Run and have it collect every repository in the workspace dynamically, so that I never have to maintain a manual repo list.
5. As an auditor, I want each collected repo's project (key/name) captured alongside it, so that a future dashboard can group repos by project as intended.
6. As an auditor, I want every user's Direct grant on every repo collected, so that I can see individually-assigned access.
7. As an auditor, I want every group's own grant on every repo collected as a first-class record (per ADR-0002), so that I don't lose "this group has admin access" even when I can't see its members.
8. As an auditor, I want each resolvable group's membership expanded into individual Member grant records, so that I can see exactly who benefits from a group's access.
9. As an auditor, I want a group's membership fetched only once per Run even if that group grants access to many repos, so that a single Run doesn't make redundant API calls per group per repo.
10. As an auditor, I want a repo whose permissions-config call fails to be marked FetchFailed without aborting the rest of the Run, so that one inaccessible repo doesn't cost me the whole audit (PRD 5.2).
11. As an auditor, I want a group whose membership fetch fails to be recorded with `members_resolved: false` and zero Member grants, distinct from a confirmed-empty group, so that future work can tell "unresolvable" apart from "empty."
12. As an auditor, I want a Bitbucket rate-limit response (429) on any call to be retried once before being treated as a failure, so that transient throttling doesn't needlessly cost me data for that call.
13. As an auditor, I want an outright credential rejection (401) to abort the entire Run immediately with a clear "credential rejected" error, rather than silently producing a Snapshot full of FetchFailed rows, so that I immediately understand my credential is the problem.
14. As an auditor, I want a Discovery failure (the top-level repo-list call failing) to result in no Snapshot being saved at all, so that a failed Run is never confused with "we audited everything and found nothing."
15. As an auditor, I want every successful Run to be persisted as a new, immutable, timestamped Snapshot in local SQLite, so that I have a permanent history to compare against later.
16. As an auditor, I want a Snapshot's PermissionRecords to preserve the Direct/Group/Member distinction on disk, so that a future diff can tell exactly how someone got their access.
17. As an auditor, I want a Snapshot's per-repo fetch status persisted, so that a future diff between two stored Snapshots can distinguish "repo absent from discovery" from "repo discovered but FetchFailed," not just from in-memory fixtures.
18. As an auditor, I want a Snapshot's group-membership-resolution status persisted even though nothing reads it yet, so that historical Snapshots aren't permanently missing this data once a future ticket needs it (ADR-0003).
19. As an auditor, I want all of this data stored locally in SQLite, never sent to a remote server, so that the tool remains local-first per the PRD's non-functional requirements.
20. As a developer, I want the Bitbucket HTTP client hidden behind a trait, so that collection logic can be unit tested against fake responses without any real network call or real credential.
21. As a developer, I want the SQLite storage tested against a real in-memory database rather than mocked queries, so that schema correctness is actually verified, not assumed.
22. As a developer, I want the new orchestration logic to call PD-1–PD-4's existing `normalize`/`diff` functions unmodified, so that the already-tested domain logic isn't duplicated or drifted.
23. As a developer, I want the Tauri command handlers themselves to contain no logic beyond calling the tested orchestration function, so that everything meaningful is covered by `cargo test` rather than requiring a running app to verify.

## Implementation Decisions

- **Credentials**: a `Credentials { username, app_password, workspace }` struct held in Tauri-managed state (in-memory, e.g. behind a `Mutex`), set via a `set_credentials` Tauri command driven by a minimal UI form. Never persisted to disk and never read back to the frontend. Full OS-keychain-backed encrypted persistence (PRD 5.1) is explicitly out of scope for this ticket — this is a deliberate, temporary boundary, not the final credential story.
- **`BitbucketClient` trait**: an abstraction over the Bitbucket calls `main.py` performs — list repositories for a workspace, list a repo's direct (user) permissions, list a repo's group permissions, list a group's members — each internally handling Bitbucket's cursor-style pagination (`next`) so callers receive fully materialized lists. A `reqwest`-backed implementation performs the real HTTP Basic Auth calls; tests use a fake implementation returning canned responses.
- **Failure typing**: the client distinguishes `Unauthorized` (401), `RateLimited` (429), and other/transient failures, so the orchestration layer can apply different handling to each.
- **Retry policy**: any call that comes back `RateLimited` is retried exactly once after a short fixed backoff; if it still fails, it's treated as a failure at that call's granularity (repo-level `FetchFailed`, or group-level `members_resolved: false`). `Unauthorized` is never retried.
- **Orchestration function (the test seam)**: a single function, e.g. `collect_and_store(client: &impl BitbucketClient, conn: &rusqlite::Connection, workspace: &str) -> Result<SnapshotId, RunError>`, called by the `run_now` Tauri command:
  - Calls `list_repositories`. `Unauthorized` here, or any other discovery failure, returns `RunError::DiscoveryFailed` — no Snapshot row is written (glossary: **Discovery failure**, `CONTEXT.md`).
  - For each discovered repo, captures `repo_project`/`repo`, fetches direct permissions and group permissions, and for each not-yet-seen group in this Run, fetches its members once and caches the result by group id for reuse across repos.
  - Feeds each repo's raw responses into the existing `normalize_repo_permissions`/group-normalization functions from PD-2–PD-4, unmodified.
  - A per-repo or per-group fetch failure (after the single retry) is recorded as `RepoFetchStatus::FetchFailed` or `GroupMembershipStatus { members_resolved: false }` respectively, and does not abort the Run — matches PRD 5.2's "partial snapshot survives."
  - All resulting `PermissionRecord`, `RepoFetchStatus`, and `GroupMembershipStatus` rows across every discovered repo are written as one new Snapshot inside a single SQLite transaction.
- **SQLite schema** (new, via `rusqlite`, bundled to avoid a system `libsqlite3` dependency):
  - `snapshots(id INTEGER PRIMARY KEY AUTOINCREMENT, run_at TEXT NOT NULL)` — `run_at` as RFC3339.
  - `permission_records(id INTEGER PRIMARY KEY AUTOINCREMENT, snapshot_id INTEGER NOT NULL REFERENCES snapshots(id), repo_project TEXT, repo TEXT, principal_id TEXT, principal_label TEXT, access_type TEXT, permission TEXT)` — `access_type` encoded as `"direct"`, `"group"`, or `"member:<group_id>"`.
  - `repo_fetch_statuses(id INTEGER PRIMARY KEY AUTOINCREMENT, snapshot_id INTEGER NOT NULL REFERENCES snapshots(id), repo_project TEXT, repo TEXT, status TEXT)` — `"ok"` or `"fetch_failed"`.
  - `group_membership_statuses(id INTEGER PRIMARY KEY AUTOINCREMENT, snapshot_id INTEGER NOT NULL REFERENCES snapshots(id), repo_project TEXT, repo TEXT, group_id TEXT, members_resolved INTEGER)` — persisted now per ADR-0003, ahead of any consumer.
  - Loading a Snapshot back out reconstructs `Vec<PermissionRecord>` + `Vec<RepoFetchStatus>` in the exact shape `diff.rs`'s existing `Snapshot`/diff function already consumes — no changes to `diff.rs`.
- **Tauri command surface**: `set_credentials(username, app_password, workspace)` (writes to managed state, no return value beyond success/error) and `run_now()` (reads managed state, invokes `collect_and_store` with the real `BitbucketClient` and the app's SQLite connection, returns the new `SnapshotId` or a serializable `RunError`). No command reads the app password back out to the frontend.
- **Repo project capture**: read directly from the same repository-list response already fetched for discovery (`project.key`/`project.name`) — no additional endpoint, per PRD 5.2.

## Testing Decisions

- Tests only exercise external behavior through the `collect_and_store` seam (fake `BitbucketClient` + real in-memory `:memory:` SQLite connection via `rusqlite`) and through the SQLite schema's round-trip shape — never implementation details of how retries or caching are wired internally.
- Cases covered: a multi-repo/multi-group/multi-member happy path produces the correct rows across all four tables; a repo-level `FetchFailed` doesn't abort the Run and unrelated repos are still collected; a group-membership fetch failure produces `members_resolved: false` with zero Member records, Run continues; a `401` on discovery aborts before any Snapshot row exists; a `401` on a later call also aborts the whole Run (credential is dead workspace-wide, not repo-specific); a `429` triggers exactly one retry then either succeeds or degrades to a failure at that call's granularity; a group granting access to multiple repos in one Run has its members fetched exactly once (cache-hit assertion on the fake client's call count); a Snapshot loaded back out of SQLite produces diff output via the existing `diff.rs` function identical to equivalent in-memory PD-2–PD-4 fixtures.
- The `set_credentials`/`run_now` `#[tauri::command]` wrappers are deliberately left untested directly (no JS test runner is configured per CLAUDE.md) — they stay thin enough that all meaningful logic lives in `collect_and_store`, covered by `cargo test`.
- Prior art: the `#[cfg(test)] mod tests` inline pattern already used in `normalize.rs`/`diff.rs`, extended to new modules for collection orchestration and storage.

## Out of Scope

- OS-keychain-backed encrypted credential persistence (PRD 5.1's full requirement) — this ticket's credential is in-memory-only and lost on app restart; a dedicated follow-up ticket owns real encryption-at-rest.
- Roster/diff dashboard UI and the execution-history page — separate tickets, per PD-1's own out-of-scope list.
- Effective-permission (cross-record) escalation detection and membership-visibility-flap detection — deferred per ADR-0001 and PD-1 user story 17; unaffected by this ticket.
- Snapshot pruning or retention limits — PRD 5.3 requires keeping everything indefinitely; no v1 mechanism needed.
- Any scheduler/cron-triggered Run — manual "Run now" only, per PRD 5.2.
- Backoff strategy beyond a single fixed retry on 429 — can be revisited if real-world rate-limiting proves this insufficient.
- Rotating the credential currently hardcoded in `main.py` — unrelated cleanup, already called out as out of scope in PD-1.

## Further Notes

- This ticket resolves PRD §8's open question about rate-limit/expired-credential behavior mid-run — see the 401-vs-429 handling above. The PRD should be updated to reflect this as a documentation follow-up, the same way PD-1 flagged its own scope against §5.2–§5.4.
- Builds directly on PD-1 through PD-4 without modifying `normalize.rs`/`diff.rs` — `collect_and_store` calls those pure functions as-is.
- A new glossary term, **Discovery failure** (distinct from the existing **Fetch failure**), was added to `CONTEXT.md` during this ticket's design session. `docs/adr/0003-persist-membership-status-ahead-of-use.md` documents the decision to persist `GroupMembershipStatus` despite it being unread by the diff engine in v1.