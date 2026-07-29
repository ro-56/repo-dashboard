# Roster tree computed per request, not materialized

Status: accepted

The roster/diff dashboard needs, for a chosen Run pair, a project → repo → Principal tree with Grant/Revoke/Level-change markers already attached. We considered two shapes: the frontend fetching snapshots and a raw diff list separately and assembling/grouping the tree in Svelte, versus a single Tauri command that loads both Snapshots' full `PermissionRecord`/`RepoFetchStatus` sets, runs the existing diff engine, and returns the fully grouped, diff-annotated tree ready to render. We chose the latter: one command, computed fresh on every request, with no materialized/cached diff stored anywhere.

Consequence, accepted deliberately for v1: every dashboard view (including selecting the same run twice for a pure roster) re-loads two full Snapshots and re-runs the diff engine, rather than reading from a precomputed diff table. This keeps grouping/sorting logic in Rust — testable via `cargo test`, not duplicated in TypeScript — and matches PD-1's existing "diff computed at query time, not stored" design. If per-request full-snapshot diffing proves too slow at real workspace scale, a materialized diff cache can be added later without changing the Rust diff engine itself, only the storage layer in front of it.
