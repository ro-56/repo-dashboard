---
name: setup-skills
description: Configure this repo's domain documentation layout — CONTEXT.md and ADR directories. Run once before first use of the domain-modeling skill.
disable-model-invocation: true
---

# Setup Skills

Scaffold the per-repo domain documentation that the engineering skills assume:

- **Domain docs** — where `CONTEXT.md` and ADRs live, and the consumer rules for reading them

This is a prompt-driven skill, not a deterministic script. Explore, present what you found, confirm with the user, then write.

## Process

### 1. Explore

Look at the current repo to understand its starting state:

- `CONTEXT.md` and `CONTEXT-MAP.md` at the repo root
- `docs/adr/` and any `src/*/docs/adr/` directories
- `CLAUDE.md` or `AGENTS.md` at the repo root — does either exist?
- Monorepo signals — a `pnpm-workspace.yaml`, a `workspaces` field in `package.json`, or a populated `packages/*` with its own `src/`. Present only in a genuinely large multi-package repo; their absence means single-context, which is almost every repo.

### 2. Present findings and decide layout

Default to **single-context** — one `CONTEXT.md` + `docs/adr/` at the repo root. This fits almost every repo; write it without asking.

Offer **multi-context** — a root `CONTEXT-MAP.md` pointing to per-context `CONTEXT.md` files — only when exploration found monorepo signals. Then confirm which layout they want.

### 3. Confirm and edit

Show the user a draft of:

- The `## Domain docs` block to add to whichever of `CLAUDE.md` / `AGENTS.md` is being edited
- A summary of the layout chosen

Let them edit before writing.

### 4. Write

**Pick the file to edit:**

- If `CLAUDE.md` exists, edit it.
- Else if `AGENTS.md` exists, edit it.
- If neither exists, ask the user which one to create — don't pick for them.

If a `## Domain docs` block already exists in the chosen file, update its contents in-place rather than appending a duplicate.

The block:

```markdown
## Domain docs

[one-line summary of layout — "single-context" or "multi-context"]. See `docs/agents/domain.md`.
```

Then write `docs/agents/domain.md` using the seed template in this skill folder:

- [domain.md](./domain.md) — domain doc consumer rules + layout

Create any missing directories lazily (`docs/agents/`).

### 5. Done

Tell the user the setup is complete. Mention they can edit `docs/agents/domain.md` directly later — re-running this skill is only necessary if they want to switch between single-context and multi-context layouts.
