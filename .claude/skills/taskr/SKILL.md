---
name: taskr
description: Operate taskr, a CLI issue tracker. Use when creating, updating, or querying tickets in a taskr-initialized project (has a .tickets/ directory).
---

# taskr

## Setup

```bash
taskr init [--prefix TKT]   # creates .tickets/ — re-run is safe (no-op)
```

## Commands

```bash
# Create
taskr add "<title>" [--type bug|feature|task|epic|chore] [--priority 0-3] [--mode afk|hitl] [--tags a,b] [--body "markdown"]

# Inspect
taskr show <id>              # frontmatter + body + dependency context
taskr list [--status open|in_progress|closed] [--count] [--tags]
taskr ls                     # alias for list
taskr check                  # validate invariants (dangling refs, link symmetry)

# Workflow
taskr start <id>             # open → in_progress
taskr close <id> [--summary "text"]   # → closed, moves to archive
taskr update <id> [--title "..."] [--priority 0-3] [--mode afk|hitl] [--tags a,b] [--body "markdown"]

# Dependencies (directional, blocking)
taskr link <dependent> <depends-on>   # A blocks until B is closed
taskr unlink <dependent> <depends-on>
taskr prune <id>             # remove ticket from all dep lists

# Related links (symmetric, non-blocking)
taskr relate <id1> <id2> [<id3> ...]  # links all pairs; accepts 2+ IDs

# Work queue
taskr ready [--mode afk]    # non-blocked tickets, sorted by priority then recency
taskr blocked               # tickets waiting on open/in_progress deps
taskr dep-tree <id> [--full] # ASCII tree of deps (--full = recursive)
```

## Notes

- All `<id>` args accept partial IDs (prefix-match); ambiguity prints matches and errors
- Priority: `0` = critical (highest), `3` = low; default `2`
- Type default: `task`; mode default: `hitl`
- `--body` on `taskr update` replaces the entire body (empty string clears it)
- `--tags` on `taskr update` replaces the entire tags list
- `taskr close --summary` is optional
- `taskr prune` removes only `dependencies` entries, not `links` (related)