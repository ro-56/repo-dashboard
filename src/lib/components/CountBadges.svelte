<script lang="ts">
  import type { CountBadge } from "$lib/repoCard";

  let { badges }: { badges: CountBadge[] } = $props();
</script>

<span class="badges">
  {#each badges as badge (badge.kind + badge.label)}
    <span class="chip chip-{badge.kind}" title={badge.title}>{badge.label}</span>
  {/each}
</span>

<style>
  .badges {
    display: flex;
    gap: var(--s-5);
    align-items: center;
    min-width: 0;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    height: var(--h-chip);
    padding: 0 var(--s-5);
    border-radius: var(--r-chip);
    font-family: var(--font-mono);
    font-weight: 600;
    font-size: var(--t-meta);
    white-space: nowrap;
  }
  .chip-added {
    background: var(--state-added-bg);
    color: var(--state-added);
  }
  .chip-removed {
    background: var(--state-revoked-bg);
    color: var(--state-revoked);
  }
  .chip-modified {
    background: var(--state-changed-bg);
    color: var(--state-changed);
  }
  .chip-esc {
    border: 1px solid var(--escalation-edge);
    background: var(--escalation-bg);
    color: var(--escalation-ink);
  }
  .chip-none {
    color: var(--ink-3);
    font-weight: 400;
  }
</style>
