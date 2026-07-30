# Dashboard is the entrypoint; credentials and Run now move into a side drawer

Status: accepted

PD-5/PD-7 put credential entry and "Run now" on a bare `/` landing page, separate from `/dashboard`, and deliberately kept credentials in-memory only (`credentials.rs`) so the form was blank on every launch. We're replacing that: `/dashboard` becomes the sole entrypoint, the separate landing route is removed, and credentials + the run trigger ("get all data") + future admin actions (snapshot deletion, etc.) live in a side drawer opened from a HeadBar control. Credentials move to OS-keychain-backed storage so they survive restart, per PRD §5.1 — the drawer's idle state is "connected as `username` on `workspace`," not a blank form.

A side drawer was chosen over a modal because the drawer's contents are expected to grow (credentials, run, snapshot management) without stacking dialogs. Triggering a run from the drawer always jumps the Baseline/Comparison selection to the new latest-vs-previous pair, even if the user had manually pinned two specific historical snapshots — simplicity over preserving a pin, since the alternative requires tracking "was this pair auto-derived or user-chosen" as separate state.
