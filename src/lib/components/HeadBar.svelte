<script lang="ts">
  import type { ComparisonSummary, PairStats, SnapshotSummary } from "$lib/roster";
  import { selectorOptions, snapshotSeqs, spanNote } from "$lib/headBar";

  let {
    snapshots,
    baselineId,
    comparisonId,
    comparison,
    pair,
    same,
    onSelectBaseline,
    onSelectComparison,
    onSwap,
  }: {
    snapshots: SnapshotSummary[];
    baselineId: number;
    comparisonId: number;
    comparison: ComparisonSummary;
    pair: PairStats;
    same: boolean;
    onSelectBaseline: (id: number) => void;
    onSelectComparison: (id: number) => void;
    onSwap: () => void;
  } = $props();

  let seqs = $derived(snapshotSeqs(snapshots));
  let options = $derived(selectorOptions(snapshots, seqs));
  let baseline = $derived(snapshots.find((s) => s.id === baselineId));
  let comparisonSnapshot = $derived(snapshots.find((s) => s.id === comparisonId));
  let note = $derived(
    baseline && comparisonSnapshot ? spanNote(baseline, comparisonSnapshot, comparison, pair, same) : "",
  );

  function handleBaselineChange(event: Event) {
    onSelectBaseline(Number((event.target as HTMLSelectElement).value));
  }
  function handleComparisonChange(event: Event) {
    onSelectComparison(Number((event.target as HTMLSelectElement).value));
  }
</script>

<header class="head-bar">
  <div class="wordmark">
    <span class="brand">perm-diff</span>
    <span class="eyebrow">repository access audit</span>
  </div>
  <div class="selectors">
    <span class="selector-label">baseline</span>
    <select class="selector" value={baselineId} onchange={handleBaselineChange}>
      {#each options as option (option.id)}
        <option value={option.id}>{option.label}</option>
      {/each}
    </select>
    <button type="button" class="swap" onclick={onSwap} aria-label="Swap baseline and comparison">⇄</button>
    <span class="selector-label">comparison</span>
    <select class="selector" value={comparisonId} onchange={handleComparisonChange}>
      {#each options as option (option.id)}
        <option value={option.id}>{option.label}</option>
      {/each}
    </select>
  </div>
  <span class="span-note">{note}</span>
</header>

<style>
  .head-bar {
    display: flex;
    align-items: center;
    gap: var(--s-8);
    height: var(--bar-head);
    flex: none;
    padding: 0 var(--s-7);
    border-bottom: 1px solid var(--rule);
    background: var(--surface);
  }

  .wordmark {
    display: flex;
    align-items: baseline;
    flex: none;
    gap: var(--s-5);
  }
  .brand {
    font-family: var(--font-mono);
    font-weight: 600;
    font-size: var(--t-head);
    letter-spacing: -0.01em;
    color: var(--ink);
  }
  .eyebrow {
    font-family: var(--font-sans);
    font-weight: 500;
    font-size: var(--t-tag);
    letter-spacing: 0.11em;
    text-transform: uppercase;
    color: var(--ink-mute);
  }

  .selectors {
    display: flex;
    align-items: center;
    flex: none;
    gap: var(--s-6);
  }
  .selector-label {
    font-family: var(--font-sans);
    font-weight: 500;
    font-size: var(--t-tag);
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--ink-mute);
  }
  .selector {
    display: inline-flex;
    align-items: center;
    height: 22px;
    min-width: 196px;
    padding: 0 var(--s-6);
    border: 1px solid var(--control-edge);
    border-radius: var(--radius);
    background: var(--surface);
    font-family: var(--font-mono);
    font-size: var(--t-body);
    color: var(--ink);
  }
  .swap {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border: 1px solid var(--control-edge);
    border-radius: var(--radius);
    background: var(--surface-sunken);
    color: var(--ink-3);
    font-family: var(--font-sans);
    font-size: var(--t-body-s);
    cursor: pointer;
  }

  .span-note {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: var(--t-body-s);
    color: var(--ink-mute);
  }
</style>
