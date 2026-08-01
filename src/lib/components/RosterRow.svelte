<script lang="ts">
  import type { PrincipalEntry } from "$lib/roster";
  import { deriveRow, editTargetForEntry, pendingRowView, rowMenu, sourceLabel, sourceTooltip } from "$lib/rosterRow";
  import { editKey, type EditableLevel, type PendingEdits, type StagedEdit } from "$lib/pendingEdits";

  let {
    entry,
    repoProject,
    repo,
    pending,
    editingEnabled,
    onStage,
    onUndo,
  }: {
    entry: PrincipalEntry;
    repoProject: string;
    repo: string;
    pending: PendingEdits;
    editingEnabled: boolean;
    onStage: (edit: StagedEdit) => void;
    onUndo: (key: string) => void;
  } = $props();

  // Direct/Repo entries only (editTargetForEntry) — Group, Project-scope, and Member rows get
  // no menu in this ticket (PD-60).
  let target = $derived(editTargetForEntry(entry));
  let key = $derived(target ? editKey(entry.scope, target, repoProject, repo) : null);
  let staged = $derived(key ? pending.get(key) : undefined);
  let row = $derived(deriveRow(entry, pendingRowView(staged)));
  let source = $derived(sourceLabel(entry.accessType, entry.scope));
  let sourceTitle = $derived(sourceTooltip(entry.accessType, entry.scope));
  // Menus are absent, not just disabled, while the Comparison isn't the latest Snapshot
  // (ADR-0022) — editing against a historical view would mutate the present based on the past.
  let menu = $derived(editingEnabled ? rowMenu(entry, staged) : null);

  let menuOpen = $state(false);
  function toggleMenu() {
    menuOpen = !menuOpen;
  }
  function closeMenu() {
    menuOpen = false;
  }

  // Re-selecting the row's true current level is treated as "undo this edit" rather than
  // staging a no-op SetLevel — matches the reference design's setLevel behaviour.
  function selectLevel(level: EditableLevel) {
    if (!target || !key) return;
    menuOpen = false;
    if (level === entry.permission) {
      onUndo(key);
      return;
    }
    onStage({
      request: { scope: entry.scope, target, repoProject, repo, action: { type: "SetLevel", level } },
      principalLabel: entry.principal.label,
      beforeLevel: entry.permission,
    });
  }

  function selectRemove() {
    if (!target || !key) return;
    menuOpen = false;
    onStage({
      request: { scope: entry.scope, target, repoProject, repo, action: { type: "Remove" } },
      principalLabel: entry.principal.label,
      beforeLevel: entry.permission,
    });
  }

  function handleUndo() {
    if (key) onUndo(key);
  }
</script>

<div
  class="roster-row"
  class:state-added={row.state === "added"}
  class:state-removed={row.state === "removed"}
  class:state-modified={row.state === "modified"}
  class:state-same={row.state === "same"}
  class:escalation={row.isEscalation}
>
  <span class="sigil">{row.sigil}</span>
  <span class="meter {row.levelClass}" class:struck={row.struck}>
    <span></span><span></span><span></span>
  </span>
  <span class="level-word {row.levelClass}" class:struck={row.struck}>{row.levelWord}</span>
  <span class="source" title={sourceTitle}>{source}</span>
  <span class="user">{entry.principal.label}</span>
  <span class="transition">
    {#if row.showTransition}
      <span class="from">{row.from}</span>
      <span class="arrow">→</span>
      <span class="to">{row.to}</span>
    {/if}
  </span>
  <span class="tags">
    {#each row.tags as tag (tag.label)}
      <span class="tag tag-{tag.kind}">{tag.label}</span>
    {/each}
    {#if staged}
      <button type="button" class="undo" onclick={handleUndo}>↺ undo</button>
    {/if}
    {#if menu}
      <span class="menu-wrap">
        <button
          type="button"
          class="menu-trigger"
          aria-haspopup="true"
          aria-expanded={menuOpen}
          aria-label="Edit permission"
          onclick={toggleMenu}
        >
          ⋯
        </button>
        {#if menuOpen}
          <div class="menu-scrim" onclick={closeMenu} aria-hidden="true"></div>
          <div class="menu">
            <div class="menu-levels">
              {#each menu.levelOptions as opt (opt.level)}
                <button type="button" class="level-opt" class:active={opt.active} onclick={() => selectLevel(opt.level)}>
                  {opt.level}
                </button>
              {/each}
            </div>
            <span class="menu-divider"></span>
            <button type="button" class="menu-remove" onclick={selectRemove}>✕ Remove access</button>
          </div>
        {/if}
      </span>
    {/if}
  </span>
</div>

<style>
  .roster-row {
    box-sizing: border-box;
    display: grid;
    grid-template-columns: var(--roster-cols);
    gap: var(--s-6);
    align-items: center;
    height: var(--row-h);
    padding: 0 var(--s-7);
    border-bottom: 1px solid var(--rule-row);
    border-left: 3px solid transparent;
    font-family: var(--font-mono);
    font-size: var(--t-body-s);
    color: var(--ink);
  }

  .roster-row.state-added {
    border-left-color: var(--state-added);
  }
  .roster-row.state-removed {
    border-left-color: var(--state-revoked);
  }
  .roster-row.state-modified {
    border-left-color: var(--state-changed);
  }
  .roster-row.escalation {
    background: var(--escalation-row);
  }

  .sigil {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--h-sigil);
    height: var(--h-sigil);
    border-radius: var(--r-chip);
    font-weight: 600;
    font-size: var(--t-id);
    color: var(--ink-3);
  }
  .state-added .sigil {
    background: var(--state-added-bg);
    color: var(--state-added);
  }
  .state-removed .sigil {
    background: var(--state-revoked-bg);
    color: var(--state-revoked);
  }
  .state-modified .sigil {
    background: var(--state-changed-bg);
    color: var(--state-changed);
  }
  .escalation .sigil {
    background: var(--escalation-bg);
    color: var(--escalation-ink);
  }

  /* Load-bearing shape channel for permission level, not diff state (ADR-0005) — fill
     color comes from the ink ramp, never a hue. */
  .meter {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .meter span {
    width: var(--seg);
    height: var(--seg);
    border-radius: 2px;
    background: var(--segment-off);
  }
  .meter.struck {
    opacity: 0.5;
  }
  .meter.lvl-admin span {
    background: var(--ink);
  }
  .meter.lvl-write span:nth-child(-n + 2) {
    background: var(--ink-2);
  }
  .meter.lvl-read span:nth-child(1) {
    background: var(--ink-4);
  }

  .level-word {
    font-family: var(--font-mono);
  }
  .level-word.struck {
    text-decoration: line-through;
    text-decoration-color: var(--strike);
    opacity: 0.7;
  }

  .lvl-admin {
    font-weight: 600;
    color: var(--ink);
  }
  .lvl-write {
    font-weight: 500;
    color: var(--ink-2);
  }
  .lvl-read {
    font-weight: 400;
    color: var(--ink-4);
  }

  .source {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink-3);
  }

  .user {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--t-id);
    color: var(--ink);
  }

  .transition {
    display: flex;
    align-items: baseline;
    gap: var(--s-3);
    white-space: nowrap;
  }
  .from {
    color: var(--ink-mute);
  }
  .arrow {
    font-weight: 600;
  }
  .state-added .arrow {
    color: var(--state-added);
  }
  .state-removed .arrow {
    color: var(--state-revoked);
  }
  .state-modified .arrow {
    color: var(--state-changed);
  }
  .to {
    color: var(--ink);
  }

  .tags {
    display: flex;
    gap: var(--s-3);
    align-items: center;
    min-width: 0;
  }
  .tag {
    display: inline-flex;
    align-items: center;
    height: var(--h-tag);
    padding: 0 var(--s-3);
    border-radius: var(--r-chip);
    font-family: var(--font-sans);
    font-size: var(--t-tag);
    font-weight: 500;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    white-space: nowrap;
    border: 1px solid;
  }
  .tag-esc {
    background: var(--tag-esc-bg);
    border-color: var(--tag-esc-border);
    color: var(--tag-esc-ink);
  }
  .tag-unresolved {
    background: var(--tag-neutral-bg);
    border-color: var(--tag-neutral-border);
    color: var(--tag-neutral-ink);
  }
  .tag-pending-level {
    background: var(--state-changed-bg);
    border-color: var(--state-changed);
    color: var(--state-changed);
  }
  .tag-pending-remove {
    background: var(--state-revoked-bg);
    border-color: var(--state-revoked);
    color: var(--state-revoked);
  }

  .undo {
    flex: none;
    border: none;
    padding: 0;
    background: none;
    font-family: var(--font-sans);
    font-size: var(--t-tag);
    color: var(--ink-3);
    cursor: pointer;
  }

  .menu-wrap {
    position: relative;
    flex: none;
    margin-left: auto;
  }
  .menu-trigger {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 20px;
    border: none;
    border-radius: var(--r-chip);
    background: transparent;
    color: var(--ink-mute);
    font-size: var(--t-body);
    line-height: 1;
    cursor: pointer;
    opacity: 0.6;
  }
  .menu-trigger:hover {
    opacity: 1;
    background: var(--surface-sunken);
  }
  .menu-scrim {
    position: fixed;
    inset: 0;
    z-index: 25;
    background: transparent;
  }
  .menu {
    position: absolute;
    top: 24px;
    right: 0;
    z-index: 30;
    display: flex;
    flex-direction: column;
    gap: var(--s-4);
    min-width: 190px;
    padding: var(--s-5);
    border: 1px solid var(--rule);
    border-radius: var(--r-card);
    background: var(--surface);
    box-shadow: var(--shadow-drawer);
  }
  .menu-levels {
    display: flex;
    gap: var(--s-2);
  }
  .level-opt {
    flex: 1;
    padding: var(--s-3) 0;
    border: 1px solid var(--control-edge);
    border-radius: var(--r-chip);
    background: transparent;
    color: var(--ink);
    font-family: var(--font-sans);
    font-weight: 600;
    font-size: var(--t-body-s);
    text-align: center;
    cursor: pointer;
  }
  .level-opt.active {
    border-color: var(--ink);
    background: var(--ink);
    color: var(--surface);
  }
  .menu-divider {
    height: 1px;
    background: var(--rule-row);
  }
  .menu-remove {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    border: none;
    padding: var(--s-2);
    border-radius: var(--r-chip);
    background: none;
    font-family: var(--font-sans);
    font-weight: 600;
    font-size: var(--t-body-s);
    color: var(--state-revoked);
    text-align: left;
    cursor: pointer;
  }

  /* Reflow order (ADR-0009): the from-level is the last thing to shed, since it's redundant
     with the row's meter and type weight (ADR-0005). The grant-source column never sheds. */
  @media (max-width: 950px) {
    .from {
      display: none;
    }
  }
</style>
