---
created: "2026-07-29T00:08:07Z"
dependencies: []
id: PD-6
links: []
mode: afk
priority: 2
status: closed
tags: []
title: 'Storage foundation: SQLite schema + Snapshot persistence'
type: task
updated: "2026-07-29T00:21:57Z"
---

## What to build

SQLite schema for `snapshots`, `permission_records`, `repo_fetch_statuses`, and `group_membership_statuses` (the last persisted ahead of any consumer, per ADR-0003), plus pure `save_snapshot`/`load_snapshot` functions that consume/produce the existing PD-1–PD-4 types (`Principal`, `PermissionRecord`, `RepoFetchStatus`, `GroupMembershipStatus`) unmodified. No HTTP, no Tauri command, no UI — this is a prefactor step that makes the Direct-grant Run-now tracer bullet (next ticket) the easy change.

## Acceptance criteria

- [ ] `rusqlite` (bundled) added as a dependency; schema for all four tables created via an init/migration routine.
- [ ] `snapshots` stores `id` + `run_at` (RFC3339) and is the parent of the other three tables via a `snapshot_id` foreign key.
- [ ] `permission_records` preserves the Direct/Group/Member(`group_id`) `access_type` distinction losslessly on write and read.
- [ ] `repo_fetch_statuses` preserves `Ok` vs `FetchFailed` distinctly from a repo simply being absent from a snapshot's rows entirely.
- [ ] `group_membership_statuses` persists `members_resolved` even though no consumer reads it yet (ADR-0003).
- [ ] `save_snapshot` writes a full snapshot (all four tables) inside a single transaction.
- [ ] `load_snapshot(id)` reconstructs `Vec<PermissionRecord>` + `Vec<RepoFetchStatus>` in the exact shape `diff.rs`'s existing `Snapshot`/diff function already consumes, with no changes to `diff.rs`.
- [ ] Round-tripping a hand-built snapshot fixture through `save_snapshot` then `load_snapshot`, then diffing it against a second fixture via the existing diff function, produces output identical to the equivalent in-memory-only PD-2–PD-4 fixtures.
- [ ] Covered by `cargo test` using a real in-memory (`:memory:`) SQLite connection — no mocked queries.
## Notes

Added rusqlite-backed storage.rs: schema for snapshots/permission_records/repo_fetch_statuses/group_membership_statuses, pure save_snapshot/load_snapshot round-tripping the existing PD-1-4 types losslessly, 6 new tests using a real :memory: connection including a diff-equivalence test against diff.rs.
