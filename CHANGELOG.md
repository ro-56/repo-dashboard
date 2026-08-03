# Changelog

All notable changes to this project are documented here. Format loosely follows [Keep a Changelog](https://keepachangelog.com/).

## [1.1.0] - 2026-08-03

### Added
- Principal search in the Roster FilterBar: narrows the tree to repos containing at least one Principal (Direct, Group, or Member) whose label matches the typed text, AND-combined with the existing view mode; a repo kept visible by a match auto-expands, and a clear-search button resets it.
- App version shown at the bottom of the side drawer.

### Changed
- A search match now narrows a repo's visible rows to only the matching Principal(s), rather than showing every row in the repo.

## [1.0.0] - 2026-08-02

First release candidate.

### Added
- Workspace-wide permission discovery: fetches every repo and Project in a configured Bitbucket Cloud workspace, resolving direct, group, and member-derived grants.
- Immutable, timestamped Snapshots of each Run, with diffing between any two Snapshots (Grant / Revoke / Level change, Escalation vs. Demotion).
- Roster dashboard: project → repo → Principal tree, filterable, with escalation highlighting and Repo/Project scope encoding.
- Targeted Refresh (re-fetch a subset of repos/Projects without full re-discovery).
- Pending edits: stage Level-change/Remove edits against Direct or Group grants and apply them as a batch.
- CSV Roster export (one-off, not a recurring reporting path).
- OS-keychain-backed credential storage (macOS Keychain, Windows Credential Store, Linux secret-service).
- Snapshot management: list and delete past Snapshots.
- Localization: English and pt-BR UI catalogs (`svelte-i18n`), with backend errors and CSV export intentionally kept English-only.
- Light/dark theme toggle.
