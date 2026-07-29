---
created: "2026-07-29T00:09:16Z"
dependencies:
    - PD-6
id: PD-7
links: []
mode: afk
priority: 2
status: closed
tags: []
title: Credential entry + Direct-grant Run now (tracer bullet)
type: task
updated: "2026-07-29T00:43:34Z"
---

## What to build

The full PRD 5.1+5.2 pipe, narrowed to Direct grants only (mirrors PD-2's role for the domain model). A minimal UI credential form → `set_credentials` Tauri command storing `Credentials { username, app_password, workspace }` in Tauri-managed in-memory state (never disk) → `run_now` Tauri command → a `BitbucketClient` trait plus a real `reqwest`-backed implementation covering repo discovery and direct-permissions calls with pagination → `ClientError` typing distinguishing `Unauthorized`/`RateLimited`/other, with a single retry+backoff on `RateLimited` and immediate abort (no retry) on `Unauthorized` → a `collect_and_store` orchestration function wired to `save_snapshot` (previous ticket), feeding the existing `normalize_repo_permissions` (PD-2) unmodified.

## Acceptance criteria

- [ ] Credential form lets the user enter/replace username, app password, and workspace; values are held in Tauri-managed in-memory state only — never written to disk, never returned to the frontend by any command.
- [ ] `run_now` dynamically discovers every repo in the configured workspace (no hardcoded repo list), capturing each repo's project (key/name) from the same list response.
- [ ] Direct permissions are fetched per repo and normalized via the existing `normalize_repo_permissions` (PD-2) unmodified.
- [ ] A repo whose direct-permissions call fails (after one retry, if `RateLimited`) is recorded as `RepoFetchStatus::FetchFailed` and the Run continues to the next repo.
- [ ] A 401 (`Unauthorized`) on repo discovery aborts the Run immediately: no Snapshot row is written, and a clear "credential rejected" error is returned to the frontend.
- [ ] A 401 on any later call (e.g. one repo's permissions call) also aborts the entire Run, not just that repo — the credential is dead workspace-wide.
- [ ] A 429 (`RateLimited`) on any call is retried exactly once after a short fixed backoff before being treated as a failure at that call's granularity.
- [ ] Pagination (Bitbucket's `next` cursor) is followed transparently so callers receive fully materialized lists.
- [ ] A successful Run persists exactly one new Snapshot via `save_snapshot`, containing only Direct-grant `PermissionRecord`s in this ticket's scope.
- [ ] Covered by `cargo test`: `BitbucketClient` is exercised only through a fake implementation returning canned responses (no real network call in tests); `collect_and_store` is tested end-to-end against a real in-memory SQLite connection.
## Notes

Added set_credentials/run_now Tauri commands, a BitbucketClient trait + reqwest-backed RealBitbucketClient (pagination, single retry+backoff on 429, immediate abort on 401), and collect_and_store orchestration wired to save_snapshot, feeding normalize_repo_permissions (PD-2) unmodified. Covered by 6 new cargo tests against a fake client + real in-memory SQLite; 41 tests total pass. Minimal credential-form/Run-now UI added to +page.svelte.
