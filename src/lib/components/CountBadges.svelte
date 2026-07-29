<script lang="ts">
  import type { CountBadge } from "$lib/repoCard";

  let { badges }: { badges: CountBadge[] } = $props();
</script>

<span class="badges">
  {#each badges as badge (badge.kind + badge.label)}
    <span class="count count-{badge.kind}">{badge.label}</span>
  {/each}
</span>

<style>
  .badges {
    display: flex;
    gap: var(--s-5);
    align-items: center;
    font-family: var(--font-mono);
    font-weight: 500;
    font-size: var(--t-meta);
  }
  .count-added {
    color: var(--state-added);
  }
  .count-removed {
    color: var(--state-revoked);
  }
  .count-modified {
    color: var(--state-changed);
  }
  .count-esc {
    display: inline-flex;
    align-items: center;
    height: 15px;
    padding: 0 var(--s-3);
    border-radius: var(--radius);
    border: 1px solid var(--tag-esc-border);
    background: var(--tag-esc-bg);
    color: var(--tag-esc-ink);
    font-weight: 600;
  }
  .count-none {
    color: var(--ink-faint);
    font-weight: 400;
  }
</style>
