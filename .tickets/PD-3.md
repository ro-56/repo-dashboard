---
created: "2026-07-28T18:15:58Z"
dependencies:
    - PD-2
id: PD-3
links: []
mode: afk
priority: 2
status: open
tags: []
title: Extend collection + diff to Group grants
type: task
updated: "2026-07-28T18:15:58Z"
---

## What to build

Extend the model from the Direct-grant tracer bullet to cover a group's own grant as a first-class Principal (ADR-0002), independent of whether its membership is resolvable (membership expansion itself is a later ticket).

- Extend normalization to parse a repo's `permissions-config/groups` response into `PermissionRecord`s with `access_type: Group`, identified by the group's stable slug/id.
- Confirm the diff function from the tracer bullet — already generic over `access_type` — correctly treats `Group` principals the same as `Direct` ones, with no new diff logic required.

## Acceptance criteria

- [ ] `PermissionRecord` can represent a group's own grant via `access_type: Group`, identified by the group's stable id/slug (never its display name).
- [ ] A group's own grant is recorded whenever the group-permissions-config call succeeds, regardless of whether membership is resolvable.
- [ ] Diffing two snapshots produces a Grant/Revoke for a group's own grant appearing/disappearing.
- [ ] Diffing produces an Escalation/Demotion when a group's own grant level changes between runs, using the same diff function as the Direct-grant tracer bullet with no code changes beyond new fixtures — proving the generic keying design.
- [ ] Covered by `cargo test` fixtures extending the tracer bullet's suite.