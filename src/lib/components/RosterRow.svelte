<script lang="ts">
  import type { PrincipalEntry } from "$lib/roster";
  import { deriveRow, sourceLabel } from "$lib/rosterRow";

  let { entry }: { entry: PrincipalEntry } = $props();

  let row = $derived(deriveRow(entry));
  let source = $derived(sourceLabel(entry.accessType));
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
  <span class="meter {row.levelClass}" class:struck={row.struck}>{row.meter}</span>
  <span class="level-word {row.levelClass}" class:struck={row.struck}>{row.levelWord}</span>
  <span class="source" title={source}>{source}</span>
  <span class="user">{entry.principal.label}</span>
  <span class="transition">
    {#if row.showTransition}
      <span class="from">{row.from}</span>
      <span class="arrow">→</span>
      <span class="to">{row.to}</span>
      {#if row.isEscalation}
        <span class="esc-mark">↑</span>
      {/if}
    {/if}
  </span>
  <span class="tags">
    {#each row.tags as tag (tag.label)}
      <span class="tag tag-{tag.kind}">{tag.label}</span>
    {/each}
  </span>
</div>

<style>
  .roster-row {
    box-sizing: border-box;
    display: grid;
    grid-template-columns: 12px 26px 46px 70px 148px 132px 1fr;
    gap: var(--s-5);
    align-items: center;
    height: var(--row-h);
    padding: 0 var(--s-6);
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
    background: var(--escalation-bg);
  }

  .sigil {
    text-align: center;
    font-weight: 600;
    font-size: var(--t-id);
    color: var(--ink-faint);
  }
  .state-added .sigil {
    color: var(--state-added);
  }
  .state-removed .sigil {
    color: var(--state-revoked);
  }
  .state-modified .sigil {
    color: var(--state-changed);
  }

  .meter {
    /* Below --t-micro deliberately — the block glyphs are a shape, not body text. */
    font-size: 7px;
    letter-spacing: 1px;
  }
  .meter.struck {
    opacity: 0.5;
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
  .esc-mark {
    font-weight: 600;
    color: var(--escalation-ink);
  }

  .tags {
    display: flex;
    gap: var(--s-3);
    align-items: center;
  }
  .tag {
    display: inline-flex;
    align-items: center;
    height: 14px;
    padding: 0 var(--s-3);
    border-radius: var(--radius);
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
</style>
