# Applied-edit log: a new, Snapshot-decoupled audit trail for justified permission edits

Bitbucket's permissions-config API has no field for a reason, so an optional Justification typed alongside a staged Level-change/Remove edit (ADR-0022) can't ride along on the `PUT`/`DELETE` call itself. Recording it durably requires a new local entity: an Applied-edit log table in SQLite, written to only when `apply_pending_edits` reports a successful outcome for an edit — one row per successful apply, carrying repo/project, principal, scope, action, before-level, after-level (or "removed"), the optional justification text, the Apply timestamp (when the change took effect on Bitbucket, not when it was staged), and an `account` field reusing `Credentials.username` as-is (not a verified email — no schema change to `Credentials`). Failed attempts are never logged; the edit stays staged for retry and is only logged if a later retry succeeds.

The log deliberately carries no reference to the Snapshot the edit was staged against. Snapshots are freely deletable at any time (ADR-0011) as routine cleanup, unrelated to whether an edit staged against one was ever applied — a foreign key would force a choice between dangling references or cascading the justification away the moment someone deletes an old Snapshot, which would defeat the log's entire purpose as a durable audit trail. The log is a standalone mutation history, not a Snapshot-derived view.

## Considered options

- **Tie each log row to its `source_snapshot_id`** — rejected. Concretely: an edit applied and logged against Snapshot S5 with justification "removed due to offboarding," followed months later by a routine deletion of S5, would either dangle the reference or cascade-delete the justification for a permanent, already-applied access change. Either outcome undermines the feature's purpose.
- **Require the justification (mandatory field)** — rejected for this round; the field is offered but not enforced.
- **Log every apply attempt, including failures** — rejected; a failed attempt changed nothing on Bitbucket, so there's nothing yet to audit. Only a successful apply is a real event.
- **Add a dedicated `email` field to `Credentials`** — rejected; `username` already holds the login identifier used for app-password auth (often an email in practice) and the log's `account` field reuses it verbatim rather than introducing a new, separately-validated credential field.

## Consequences

- A future "browse the Applied-edit log" viewer is a natural follow-up, out of scope here — this round only surfaces the justification back in the existing Apply results view.
- `PendingEditRequest`/`StagedEdit` (frontend and `src-tauri/src/apply.rs`) gain a new optional justification field, threaded through to `apply_pending_edits`.
