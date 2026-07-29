---
name: taskr
description: Operate taskr, a CLI issue tracker. Use when creating, updating, or querying tickets in a taskr-initialized project (has a .tickets/ directory).
---

# taskr

## Before first use

Check for `.tickets/` in the project root. If absent, initialize it:

```bash
taskr init [--prefix TKT]
```

Re-running `init` on an already-initialized project is a safe no-op — don't skip this check just because you're unsure.

## Quick reference

| Goal | Command |
|---|---|
| Create a ticket | `taskr add "<title>" [flags]` |
| Read a ticket (body + deps) | `taskr show <id>` |
| List tickets | `taskr list [--status ...]` (alias: `taskr ls`) |
| Validate ticket data | `taskr check` |
| Begin work | `taskr start <id>` |
| Finish work | `taskr close <id> [--summary "..."]` |
| Edit a ticket | `taskr update <id> [flags]` |
| Add a blocking dependency | `taskr link <dependent> <depends-on>` |
| Remove a blocking dependency | `taskr unlink <dependent> <depends-on>` |
| Strip a ticket from all deps | `taskr prune <id>` |
| Add non-blocking cross-references | `taskr relate <id1> <id2> [...]` |
| See what's unblocked to work on | `taskr ready [--mode afk]` |
| See what's stuck waiting | `taskr blocked` |
| Visualize dependency chain | `taskr dep-tree <id> [--full]` |

## Creating tickets

```bash
taskr add "<title>" [--type bug|feature|task|epic|chore] [--priority 0-3] [--mode afk|hitl] [--tags a,b] [--body "markdown"]
```

- Omitted `--type` → `task`. Omitted `--mode` → `hitl`. Omitted `--priority` → `2`.
- Priority scale: `0` = critical/highest, `3` = low.

Example: `taskr add "Fix login redirect" --type bug --priority 1 --tags auth,web`

## Reading and validating

```bash
taskr show <id>              # full frontmatter + body + dependency context
taskr list [--status open|in_progress|closed] [--count] [--tags]
taskr check                  # detects dangling refs, broken link symmetry
```

Run `taskr check` after bulk edits or scripted changes to catch invariant violations before they compound.

## Moving tickets through their lifecycle

```bash
taskr start <id>                      # open → in_progress
taskr close <id> [--summary "text"]   # → closed, archives the ticket; --summary is optional
```

## Editing a ticket

```bash
taskr update <id> [--title "..."] [--priority 0-3] [--mode afk|hitl] [--tags a,b] [--body "markdown"]
```

⚠️ Destructive-by-default fields — passing these **replaces**, not merges:
- `--body` replaces the entire body (pass `""` to clear it)
- `--tags` replaces the entire tag list (not additive — include all tags you want kept)

## Managing dependencies

Two distinct relationship types — pick the right one:

- **Blocking (directional):** `A depends-on B` means A cannot proceed until B closes.
```bash
  taskr link <dependent> <depends-on>     # A blocks on B
  taskr unlink <dependent> <depends-on>   # remove that block
  taskr prune <id>                        # remove <id> from every ticket's dependency list (does NOT touch `relate` links)
```
- **Related (symmetric, non-blocking):** just a cross-reference, no ordering implied.
```bash
  taskr relate <id1> <id2> [<id3> ...]    # links every pair among the given IDs
```

## Finding what to work on

```bash
taskr ready [--mode afk]     # non-blocked tickets, sorted priority then recency
taskr blocked                # tickets still waiting on open/in_progress deps
taskr dep-tree <id> [--full] # ASCII dependency tree; --full recurses through all levels
```

## Reference notes

- Any `<id>` accepts a partial/prefix match; an ambiguous prefix prints all matches and errors instead of guessing.
- `taskr prune` only strips `dependencies` entries — `relate` links are untouched.
