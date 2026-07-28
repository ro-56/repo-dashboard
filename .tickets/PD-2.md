---
created: "2026-07-28T18:15:58Z"
dependencies: []
id: PD-2
links: []
mode: afk
priority: 2
status: open
tags: []
title: Direct-grant collection + diff (tracer bullet)
type: task
updated: "2026-07-28T18:15:58Z"
---

## What to build

The smallest complete, testable path through the Principal model defined in PD-1: collect and diff Direct grants only — no groups yet. This proves the record shape and the diff engine before any group complexity is layered on.

- Core types: `Principal`, `PermissionRecord` (with an `access_type` field designed so `Group`/`Member` variants can be added later without reshaping the record), `RepoFetchStatus`.
- A pure normalization function: given one repo's already-fetched raw `permissions-config/users` response (success or fetch failure), produce `Vec<PermissionRecord>` (Direct grants) and a `RepoFetchStatus`.
- A pure diff function: given two snapshots' worth of `PermissionRecord`s + `RepoFetchStatus`es, produce Grant / Revoke / Escalation / Demotion entries keyed on `(repo, principal_id, access_type)`, plus a repo-level distinction between "absent from discovery" and `FetchFailed`.
- Principal identity for users is Bitbucket's stable `account_id`; nickname/display name are label-only fields, never used for identity or diff keying.

## Acceptance criteria

- [ ] `PermissionRecord` records a principal's Direct grant using `account_id` as identity.
- [ ] A repo whose `permissions-config/users` call fails produces `RepoFetchStatus::FetchFailed` and zero `PermissionRecord`s for that repo — never treated as "zero grants."
- [ ] Diffing two snapshots produces a Grant for a Direct grant present only in the later snapshot.
- [ ] Diffing produces a Revoke for a Direct grant present only in the earlier snapshot.
- [ ] Diffing produces an Escalation when the same principal's Direct grant level rises (read < write < admin), and a Demotion when it falls.
- [ ] Diffing produces no diff entry when the same principal's Direct grant is unchanged between runs.
- [ ] Changing a principal's nickname/display name between two snapshots (same `account_id`) produces no diff entry.
- [ ] A repo present in both snapshots' discovery but `FetchFailed` in one is reported distinctly from a repo absent from discovery entirely.
- [ ] All of the above is covered by `cargo test` fixtures — no HTTP, SQLite, or Tauri involved.