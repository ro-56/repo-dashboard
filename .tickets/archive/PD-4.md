---
created: "2026-07-28T18:15:58Z"
dependencies:
    - PD-3
id: PD-4
links: []
mode: afk
priority: 2
status: closed
tags: []
title: Extend collection to Member grants (group membership expansion)
type: task
updated: "2026-07-28T23:29:38Z"
---

## What to build

Extend normalization so each group grant's membership is expanded into individual Member grants when resolvable, while preserving the group's own grant from the previous ticket unchanged.

- For each group grant, attempt to fetch its member list and expand it into `PermissionRecord`s with `access_type: Member(group_id)`, one per resolved member, at the group's permission level.
- Track a `members_resolved` flag per group per repo per snapshot: `true` when the member list was fetched successfully (empty or not), `false` when the fetch failed or was inaccessible. Confirmed-empty and unresolvable membership both produce zero Member records, but must be distinguishable via this flag for future work (visibility-flap detection is explicitly deferred, per PD-1).
- Confirm the diff engine treats `Member(group_id)` as just another `access_type` value, requiring no new diff logic (per ADR-0001/ADR-0002).

## Acceptance criteria

- [ ] A group with resolvable, non-empty membership produces one Member record per member, at the group's permission level.
- [ ] A group with resolvable but empty membership produces zero Member records and `members_resolved: true`.
- [ ] A group whose membership fetch fails produces zero Member records and `members_resolved: false` — never conflated with a confirmed-empty group.
- [ ] Diffing two snapshots correctly Grants/Revokes/Escalates/Demotes individual Member records using the existing diff function, with no changes beyond new fixtures.
- [ ] A principal holding both a Direct grant and a Member grant on the same repo at different levels is represented as two independent records, never collapsed (ADR-0001) — a new Member grant appearing alongside an unchanged Direct grant surfaces as a Grant, not an Escalation.
- [ ] Covered by `cargo test` fixtures extending the previous tickets' suites.
## Notes

Extended group normalization to expand resolvable membership into Member(group_id) PermissionRecords at the group's permission level, tracked members_resolved per group/repo/snapshot (Ok(empty) vs FetchFailed distinguished via a dedicated sentinel enum), and confirmed the diff engine needs no changes beyond new fixtures for Member records.
