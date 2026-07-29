---
created: "2026-07-29T00:09:26Z"
dependencies:
    - PD-7
id: PD-8
links: []
mode: afk
priority: 2
status: open
tags: []
title: Extend collection to Group grants
type: task
updated: "2026-07-29T00:09:26Z"
---

## What to build

Extend the model from the Direct-grant Run-now tracer bullet to cover Group grants (mirrors PD-3's role for the domain model). Add `list_group_permissions` (with pagination) to `BitbucketClient`; extend `collect_and_store` to fetch and normalize each repo's group grants via the existing `normalize_repo_group_permissions` (PD-3) unmodified, reusing the same retry/failure semantics already built for direct-permissions calls. No schema change — `access_type` is already a flexible column from ticket PD-6.

## Acceptance criteria

- [ ] Each repo's group permissions are fetched and normalized into `PermissionRecord`s with `access_type: Group`, identified by the group's stable slug.
- [ ] A group-permissions call failure (after one retry on `RateLimited`) is recorded the same way a direct-permissions failure is (`RepoFetchStatus::FetchFailed` for that repo), reusing the existing failure-handling path rather than new logic.
- [ ] A Run's persisted Snapshot now contains both Direct and Group grants in the same `permission_records` table with no schema migration.
- [ ] Covered by `cargo test` extending the previous ticket's fake-client and in-memory-SQLite fixtures with group-permission scenarios (success, empty, fetch failure).