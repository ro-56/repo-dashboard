---
created: "2026-07-29T00:10:03Z"
dependencies:
    - PD-8
id: PD-9
links: []
mode: afk
priority: 2
status: open
tags: []
title: Extend collection to Member grants (group membership expansion + caching)
type: task
updated: "2026-07-29T00:10:03Z"
---

## What to build

Extend collection to Member grants via group-membership expansion (mirrors PD-4's role for the domain model). Add `list_group_members` (with pagination) to `BitbucketClient`; extend `collect_and_store` so that, for each group grant encountered in a Run, its member list is fetched once (cached by group id, reused across every repo that group grants access to — the caching improvement agreed in the PD-5 design session) and expanded via the existing member-expansion logic (PD-4) unmodified; persist `members_resolved` per group per repo into the `group_membership_statuses` table built in the storage-foundation ticket.

## Acceptance criteria

- [ ] A group's member list is fetched at most once per Run, even when that group grants access to multiple repos (verified via a call-count assertion on the fake client).
- [ ] A group with resolvable, non-empty membership produces one `Member(group_id)` `PermissionRecord` per member, at the group's permission level.
- [ ] A group with resolvable but empty membership produces zero Member records and `members_resolved: true`.
- [ ] A group whose membership fetch fails (after one retry on `RateLimited`) produces zero Member records and `members_resolved: false`, and the Run continues.
- [ ] `group_membership_statuses` rows are written into the table from the storage-foundation ticket as part of the same `save_snapshot` transaction as the rest of the Run's data.
- [ ] A Run's persisted Snapshot now contains Direct, Group, and Member grants together, matching `main.py`'s data coverage plus the per-Run membership caching improvement.
- [ ] Covered by `cargo test` extending prior tickets' fixtures with member-expansion scenarios (resolved non-empty, resolved empty, unresolved) and the caching assertion.