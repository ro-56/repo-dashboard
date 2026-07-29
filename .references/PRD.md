# repo-dashboard — Product Requirements

Status: draft, based on grilling session 2026-07-28. Source material: `project-ref.md`, `main.py`, `dashboard_designs.html`.

## 1. Problem

Repository access on Bitbucket is granted/changed outside any review process. Today, checking it means running `main.py` by hand and reading an Excel file — there's no history, so nothing shows *what changed*, only a snapshot at the moment the script ran. repo-dashboard turns that one-off script into a standing local tool that keeps a history of every run and answers two questions on one screen:

- Who has access to each repository right now, and at what level?
- What changed between any two runs — grants added, revoked, or moved to a different level?

## 2. Goals

Roughly equal weight between:
- **Faster audits** — replace "run script → open Excel → eyeball it" with a dashboard you can check in seconds.
- **Catching escalations** — surface permission increases (e.g. read → admin) that shouldn't have happened, before they're a problem.

## 3. Users

Single admin / small trusted team, running the tool locally. No login system, no roles — access is whoever has the app installed and the Bitbucket credential. Do not build multi-user auth.

## 4. Scope

**One release, not phased.** Earlier framing considered splitting this into "dashboard now, trigger/credentials/history later," but the decision is to ship all four pillars together:

1. Credential entry & storage
2. Run trigger (fetch a new snapshot)
3. Roster + diff dashboard (the prototyped grouped-card IA)
4. Execution-history page

### Explicit non-goals (do not build)
- Multi-workspace or multi-provider support. Single Bitbucket Cloud workspace, hardcoded like today (`unisoma`), configurable but not multi-tenant.
- Any notification/alerting channel (email, Slack, etc.). Purely pull-based — user opens the app to check.
- Excel/CSV export. The dashboard fully replaces the Excel report; don't maintain both output paths.
- Login, roles, or any multi-user permission model for the tool itself.
- Group membership tracked as a first-class diffable entity. Match `main.py`'s current model: group grants are flattened into per-user rows before storage/diff; group origin is kept as metadata (e.g. "via group X") but is not itself a diffed dimension.

## 5. Functional requirements

### 5.1 Credentials
- Bitbucket username + app password are entered through the app UI, not env vars or source.
- Stored locally, encrypted at rest (OS keychain via Tauri, e.g. `tauri-plugin-stronghold` or the system keyring — not a plaintext file, not a hardcoded fallback like the current `main.py`).
- One credential set per workspace config; editable/replaceable from the UI.

### 5.2 Run trigger
- Manual only: a "Run now" action in the UI triggers a fetch against Bitbucket and stores the result as a new versioned snapshot with a timestamp. No scheduler/cron in v1.
- Repository discovery stays dynamic (as in `main.py`'s `listar_todos_repositorios`) — don't hardcode a repo list.
- For each repository, must additionally capture its **project** (key/name), which `main.py` does not currently fetch — required for the grouped-by-project IA (`project sections → repo cards`). This is available on the repository object from the same list-repositories call, no extra endpoint needed.
- Direct user permissions and group permissions (with group members expanded) are collected per repo, same as today, and normalized into one record shape: `username · repo_project · repo · permission (read | write | admin)`.
- A run that fails partway (e.g. one repo returns 404/"no access") should still save the runs that succeeded — don't fail the whole snapshot for one inaccessible repo (`main.py` already handles this per-repo with a "SEM ACESSO OU INACESSÍVEL" row; keep that behavior, adapted to the new data model).

### 5.3 Snapshot storage
- Every run is persisted as an immutable, timestamped snapshot — never overwritten.
- Retention: keep everything indefinitely, no pruning/archiving in v1.
- Local storage (SQLite is the natural fit given the Tauri/Rust backend), not a remote DB.

### 5.4 Roster + diff dashboard
Implements the finalized IA from `dashboard_designs.html`: project sections → repo cards, grouped by repository.

- **Collapsed card**: summarizes access-level distribution (count per read/write/admin) and a change count, for a chosen run-pair.
- **Expanded card**: full member roster for that repo, with diff markers shown in place (not a separate change feed).
- **Run selection**: two independent selectors (matches prototyped state `1g`), so any two runs can be compared, not just "latest vs previous." Selecting the same run in both = pure roster view, no diff.
- **Diff semantics**, per user+repo:
  - *Grant* — present in run B, absent in run A.
  - *Revoke* — present in run A, absent in run B.
  - *Level change* — present in both, permission differs. Ordering is `read < write < admin`; a level change is an **escalation** if it moved up, a **demotion** if down.
  - *Repository absent from comparison run* — the whole roster for that repo is marked struck-through (prototyped state `1e`), rather than silently omitted.
- **Escalation highlighting**: a togglable visual emphasis on escalations specifically (prototype exposes this as `highlight-escalations`), distinct from generic "changed."
- **Filters**: "changes only" view that hides unchanged rows/cards (prototyped state `1d`); must handle the empty-result case (`1i`) and the no-changes-between-runs case (`1h`) without looking broken.
- **All-collapsed overview**: a scan mode showing all repo cards collapsed at once (prototyped state `1b`) for a quick full-workspace glance.

### 5.5 Execution-history page
Not covered by the visual prototypes — needs its own design pass, but functional baseline:
- List of past runs: timestamp, repo count, total principals, and a change-count summary relative to the immediately preceding run.
- Selecting two runs from this list should route into the roster/diff dashboard with those runs pre-selected.
- No edit/delete of past runs in v1 (snapshots are immutable per §5.3).

## 6. Non-functional requirements
- **Local-first desktop app**: Tauri + SvelteKit, matching the existing repo scaffold. No server component, no network dependency beyond calling the Bitbucket API directly from the user's machine.
- **No auth system**: access to the app = access to the data; rely on OS-level access to the machine.
- Must not regress the one behavior that matters most from `main.py`: it should still be able to enumerate every repo in the workspace and every direct/group grant without manual repo-list maintenance.

## 7. Data model (summary)

```
Snapshot
  id, run_at (timestamp)

PermissionRecord (per snapshot)
  snapshot_id
  repo_project
  repo
  username
  display_name
  access_type      # "direct" | "group:<group_name>"
  permission        # read | write | admin
```

Diff between two snapshots is computed at query time (not stored), keyed on `(repo, username)`, same approach as the HTML prototypes ("diff computed at runtime").

## 8. Open questions (not resolved in this session)
- Execution-history page has no visual design yet — needs its own prototype/review before build.
- No decision on what happens if Bitbucket API rate-limits or the app password expires mid-run — needs a defined error/retry behavior.
- Rotate the Bitbucket app password currently hardcoded in `main.py` regardless of this rebuild — it's a live credential sitting in plaintext.

## 9. Traceability
- IA rationale and scope boundaries: `.references/project-ref.md`
- Current data-collection logic to port: `.references/main.py`
- Finalized visual design + prototyped states (`1a`–`1i`): `.references/dashboard_designs.html` (combined) or `.references/standalone/` (split one-file-per-state, e.g. `1a Default access.html`)
