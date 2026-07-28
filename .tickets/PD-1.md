---
created: "2026-07-28T17:56:50Z"
dependencies: []
id: PD-1
links: []
mode: afk
priority: 2
status: open
tags: []
title: Permission normalization & diff engine (Principal model)
type: feature
updated: "2026-07-28T17:56:50Z"
---

## Problem Statement

An auditor reviewing Bitbucket repository access today gets a flat, unstructured dump from `main.py`: user rows and group rows mixed together, group membership silently treated as "empty" whenever it can't be read, users identified by an editable nickname, and no way to tell "this repo has zero grants" apart from "we couldn't read this repo's grants at all." None of that is precise enough to build a trustworthy diff on top of — and diffing runs to catch escalations is the whole point of repo-dashboard.

## Solution

Define, and implement as pure Rust functions, the normalized shape every Bitbucket permission collection run must produce, and the diff engine that compares two such collections. A group becomes a first-class Principal — its own grant is recorded and diffable even when its membership can't be resolved. Fetch failures get their own per-repo status instead of being conflated with "no grants" or "repo doesn't exist." Users are identified by Bitbucket's stable `account_id`, not their nickname. Diffing operates strictly per grant-source record (Direct / Group / Member), not on a computed "effective permission" — a deliberate v1 simplification (see ADR-0001).

## User Stories

1. As an auditor, I want every repository's permissions collected and normalized into one common record shape, so that I can compare access across repos and runs consistently.
2. As an auditor, I want a user's Direct grant recorded distinctly from any grant they receive via a group, so that I can see exactly how someone got their access.
3. As an auditor, I want a group's own permission grant recorded even when I can't see who belongs to that group, so that I don't lose the fact that "this group has admin access" just because my credential lacks visibility into its membership.
4. As an auditor, I want each resolved group member recorded as their own grant record layered on top of the group's own grant, so that I can see both the group-level fact and the specific people it affects.
5. As an auditor, I want repos where the permissions fetch failed marked with a distinct status — not silently dropped, and not conflated with "zero grants" or "repo no longer discovered" — so that I know when I'm missing data versus when access has genuinely changed.
6. As an auditor, I want users identified by Bitbucket's stable account ID rather than their nickname, so that someone renaming their nickname between two runs never looks like a revoke-and-grant pair.
7. As an auditor, I want to compare any two runs and see, per permission record, whether it was granted, revoked, or changed level, so I can spot exactly what changed.
8. As an auditor, I want level changes classified as an Escalation (read→write→admin) or a Demotion (the reverse), so I can prioritize reviewing escalations first.
9. As an auditor, I want repos entirely absent from one of the two compared runs' discovery flagged distinctly from repos where the permissions fetch failed, so I don't confuse "repo deleted/undiscovered" with "we couldn't read its permissions."
10. As an auditor, I want a user gaining access through a brand-new group membership — while keeping an existing, unrelated Direct grant — surfaced at minimum as a new Grant, so I'm alerted even though it isn't (in v1) classified as a formal Escalation.
11. As an auditor, I want the diff engine to operate purely on two already-collected snapshots with no network calls, so comparisons are fast and repeatable regardless of current Bitbucket API availability.
12. As a developer maintaining this tool, I want the normalization logic and the diff logic implemented as pure functions independent of the Tauri/HTTP/SQLite layers, so they can be unit tested directly with `cargo test`.
13. As an auditor, I want a group's own grant to participate in the same Grant/Revoke/Escalation logic as a user's Direct grant, so a group's permission level rising from write to admin is itself caught as an escalation, even if I never resolve its members.
14. As an auditor, I want a group with zero confirmed members (successfully fetched, empty list) distinguished internally from a group whose membership couldn't be fetched at all, so future work can treat them differently even though v1's diff output doesn't yet distinguish them.
15. As an auditor, I want the normalized permission record to carry the repo's project (key/name), so repos can be grouped by project as intended by the dashboard's information architecture.
16. As a developer, I want a single `Principal` concept encompassing both users and groups, so the normalization and diff logic don't need separate code paths for "is this a user row or a group row."
17. As an auditor comparing two runs where a group's membership was visible in one run but not the other, I accept that, in v1, the resulting apparent revokes for that group's former members are a known limitation rather than a bug — visibility-change detection is deferred, not silently wrong.

## Implementation Decisions

- **`PermissionRecord` (per snapshot, per grant-source record):**
  - `principal_id` — Bitbucket `account_id`/`uuid` for a user Principal, or the group's slug/id for a group Principal. Never the nickname.
  - `principal_label` — display name / group name. Label only, never used for identity or diff keying.
  - `repo_project`, `repo`.
  - `access_type` — `Direct` | `Group` (the group's own grant) | `Member(group_id)` (a resolved member's grant via that group).
  - `permission` — `Read` | `Write` | `Admin`.
- **`RepoFetchStatus` (per snapshot, per repo):** `repo_project`, `repo`, `status` (`Ok` | `FetchFailed`). Recorded independently of `PermissionRecord` since a fetch failure has no Principal to attach to. A repo absent from a run's discovery entirely is a separate condition — it produces no `RepoFetchStatus` row at all for that snapshot, versus `FetchFailed` which means the repo was discovered but its permissions-config call errored.
- **Group membership resolution flag:** collection tracks, per group per repo per snapshot, whether membership was resolved (`members_resolved: bool`). Not consumed by the diff engine in v1 (per user story 14 / user story 17), but retained so a later iteration can distinguish "confirmed empty group" from "unresolvable membership" and detect visibility-change flapping between runs.
- **Normalization function:** pure, takes one repo's already-fetched raw API responses (user-permissions list or a fetch-failed sentinel, group-permissions list, per-group members list or an unresolved sentinel) and returns `(Vec<PermissionRecord>, RepoFetchStatus)`. No I/O.
- **Diff function:** pure, takes two snapshots' worth of `PermissionRecord`s and `RepoFetchStatus`es and returns diff entries keyed on `(repo, principal_id, access_type)`:
  - Present in B only → `Grant`.
  - Present in A only → `Revoke`.
  - Present in both, same key, different `permission` → `LevelChange` (`Escalation` if the level rose, `Demotion` if it fell; ordering `Read < Write < Admin`).
  - Present in both with identical `permission` → no diff entry.
  - Repo-level: absent-from-discovery and `FetchFailed` are reported as distinct repo-level statuses in the diff output, never merged.
- **No effective-permission computation.** Diffing never aggregates a Principal's multiple records (e.g. Direct + Member) into one "effective" level before comparing — see ADR-0001 (`docs/adr/0001-diff-per-grant-source.md`).
- **Groups are Principals.** A group's own grant is stored and diffed exactly like a user's Direct grant, independent of whether its membership resolves — see ADR-0002 (`docs/adr/0002-groups-are-principals.md`).
- Glossary/terminology throughout follows `CONTEXT.md` (Principal, Direct/Group/Member grant, Unresolvable membership, Fetch failure, Run, Snapshot, Grant/Revoke, Level change/Escalation/Demotion).

## Testing Decisions

- Tests exercise the two pure functions only through their public input/output types — no HTTP mocking, no SQLite, no Tauri command invocation, since both are designed as pure Rust functions per CLAUDE.md's guidance that backend logic is tested with `cargo test` in `src-tauri`.
- **Normalization tests**, given hand-built raw-response fixtures, assert the exact `(Vec<PermissionRecord>, RepoFetchStatus)` produced for: a plain Direct grant; a Group grant with resolved members; a Group grant with unresolved members (`members_resolved: false`, zero Member records emitted); a confirmed-empty group (`members_resolved: true`, zero Member records); a permissions-config fetch failure (`RepoFetchStatus::FetchFailed`, zero `PermissionRecord`s for that repo).
- **Diff engine tests**, given two fixed, hand-constructed record sets (not fetched), assert exact diff output for: a pure Grant; a pure Revoke; an Escalation; a Demotion; a no-op (identical record in both runs); a new Member grant appearing alongside an unrelated, unchanged Direct grant (must emit as `Grant`, never `Escalation` — proves ADR-0001); identical `principal_id` with a changed `principal_label`/nickname (must emit no diff at all — proves the stable-identity decision); a group's own grant level changing (must emit `Escalation`/`Demotion` on the group Principal itself — proves ADR-0002); a repo present in both runs' discovery but `FetchFailed` in one (must be distinguished from a repo absent from discovery entirely).
- No prior art exists in this repo yet — it's the stock Tauri scaffold with a single `greet` command and no tests. These would be the first Rust unit tests in the project.

## Out of Scope

- SQLite persistence/schema wiring for `Snapshot`/`PermissionRecord`/`RepoFetchStatus` — separate ticket.
- The actual Bitbucket HTTP client (auth, pagination, endpoint calls) — a mechanical port of `main.py`'s calls; this spec only defines the shapes normalization must consume and produce.
- Credential entry, encryption, and storage.
- Tauri command surface and frontend invocation wiring.
- Roster/diff dashboard UI, escalation-highlighting toggle, changes-only filter — the existing HTML prototypes (`1a`-`1i`) predate this model and have no state yet for a group appearing as its own roster row or for the new `FetchFailed` repo status; that needs its own design pass before UI work starts.
- Execution-history page.
- Effective-permission (cross-record) escalation detection — deferred, see ADR-0001.
- Membership-visibility-flap detection across runs (a group's members being resolvable in one run but not another) — deferred, accepted as a known v1 limitation per user story 17.
- Rotating the hardcoded Bitbucket credential currently in `main.py` — unrelated cleanup, not part of this spec.

## Further Notes

This spec formalizes decisions from a domain-modeling session; see `CONTEXT.md` for the full glossary and `docs/adr/0001-diff-per-grant-source.md` / `docs/adr/0002-groups-are-principals.md` for the two accepted architectural trade-offs. `.references/PRD.md` (§5.2-§5.4, §7) predates this model and should be updated to match as a documentation follow-up alongside or after this ticket.