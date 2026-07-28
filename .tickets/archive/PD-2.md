---
created: "2026-07-28T18:15:58Z"
dependencies: []
id: PD-2
links: []
mode: afk
priority: 2
status: closed
tags: []
title: Direct-grant collection + diff (tracer bullet)
type: task
updated: "2026-07-28T18:56:23Z"
---

## What to build

The smallest complete, testable path through the Principal model defined in PD-1: collect and diff Direct grants only — no groups yet. This proves the record shape and the diff engine before any group complexity is layered on.

- Core types: `Principal`, `PermissionRecord` (with an `access_type` field designed so `Group`/`Member` variants can be added later without reshaping the record), `RepoFetchStatus`.
- A pure normalization function: given one repo's already-fetched raw `permissions-config/users` response (success or fetch failure), produce `Vec<PermissionRecord>` (Direct grants) and a `RepoFetchStatus`.
- A pure diff function: given two snapshots' worth of `PermissionRecord`s + `RepoFetchStatus`es, produce Grant / Revoke / Escalation / Demotion entries keyed on `(repo, principal_id, access_type)`, plus a repo-level distinction between "absent from discovery" and `FetchFailed`.
- Principal identity for users is Bitbucket's stable `account_id`; nickname/display name are label-only fields, never used for identity or diff keying.

## Acceptance criteria

- [x] `PermissionRecord` records a principal's Direct grant using `account_id` as identity.
- [x] A repo whose `permissions-config/users` call fails produces `RepoFetchStatus::FetchFailed` and zero `PermissionRecord`s for that repo — never treated as "zero grants."
- [x] Diffing two snapshots produces a Grant for a Direct grant present only in the later snapshot.
- [x] Diffing produces a Revoke for a Direct grant present only in the earlier snapshot.
- [x] Diffing produces an Escalation when the same principal's Direct grant level rises (read < write < admin), and a Demotion when it falls.
- [x] Diffing produces no diff entry when the same principal's Direct grant is unchanged between runs.
- [x] Changing a principal's nickname/display name between two snapshots (same `account_id`) produces no diff entry.
- [x] A repo present in both snapshots' discovery but `FetchFailed` in one is reported distinctly from a repo absent from discovery entirely.
- [x] All of the above is covered by `cargo test` fixtures — no HTTP, SQLite, or Tauri involved.
## Notes

Implemented Principal/PermissionRecord/RepoFetchStatus model, pure normalize_repo_permissions (Direct grants + FetchFailed), and pure diff engine (Grant/Revoke/Escalation/Demotion, repo-level FetchFailed vs absent-from-discovery), keyed per PD-1 on (repo, principal_id, access_type). 12 cargo test fixtures, all passing.
