<script lang="ts">
  import type { ComparisonSummary, PairStats } from "$lib/roster";
  import { currentStateStats, deltaStats, leftClusterLabel, rightClusterLabel } from "$lib/summaryBar";

  let {
    comparison,
    pair,
    same,
    baselineSeq,
    comparisonSeq,
  }: {
    comparison: ComparisonSummary;
    pair: PairStats;
    same: boolean;
    baselineSeq: number;
    comparisonSeq: number;
  } = $props();

  let leftStats = $derived(currentStateStats(comparison));
  let rightStats = $derived(deltaStats(pair, same));
  let leftLabel = $derived(leftClusterLabel(comparisonSeq));
  let rightLabel = $derived(rightClusterLabel(same, baselineSeq));
</script>

<div class="summary-bar">
  <div class="cluster">
    <span class="cluster-label">{leftLabel}</span>
    <div class="stats">
      {#each leftStats as stat (stat.key)}
        <div class="stat">
          <span class="value tone-{stat.tone}">{stat.value}</span>
          <span class="label">{stat.label}</span>
        </div>
      {/each}
    </div>
  </div>
  <div class="cluster">
    <span class="cluster-label">{rightLabel}</span>
    <div class="stats">
      {#each rightStats as stat (stat.key)}
        <div class="stat">
          <span class="value tone-{stat.tone}">{stat.value}</span>
          <span class="label">{stat.label}</span>
        </div>
      {/each}
    </div>
  </div>
</div>

<style>
  .summary-bar {
    display: flex;
    align-items: stretch;
    height: var(--bar-summary);
    flex: none;
    border-bottom: 1px solid var(--rule);
    background: var(--surface-raised);
  }

  .cluster {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: var(--s-3);
    padding: 0 var(--s-8);
  }
  .cluster:first-child {
    border-right: 1px solid var(--rule-row);
  }

  .cluster-label {
    font-family: var(--font-sans);
    font-weight: 500;
    font-size: var(--t-tag);
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--ink-mute);
  }

  .stats {
    display: flex;
    gap: var(--s-8);
  }

  .stat {
    display: flex;
    align-items: baseline;
    gap: var(--s-3);
  }

  .value {
    font-family: var(--font-mono);
    font-weight: 600;
    font-size: var(--t-stat);
    letter-spacing: -0.02em;
    color: var(--ink);
  }
  .label {
    font-family: var(--font-sans);
    font-weight: 500;
    font-size: var(--t-tag);
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--ink-mute);
    white-space: nowrap;
  }

  .tone-admin {
    color: var(--ink);
  }
  .tone-write {
    color: var(--ink-2);
  }
  .tone-read {
    color: var(--ink-4);
  }
  .tone-added {
    color: var(--state-added);
  }
  .tone-removed {
    color: var(--state-revoked);
  }
  .tone-modified {
    color: var(--state-changed);
  }
  .tone-esc {
    color: var(--escalation-ink);
  }
  .tone-muted {
    color: var(--ink-faint);
  }
</style>
