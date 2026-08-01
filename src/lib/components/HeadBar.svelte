<script lang="ts">
  import type { ComparisonSummary, PairStats, SnapshotSummary } from "$lib/roster";
  import { deltaChips, selectorOptions, snapshotSeqs, spanNote } from "$lib/headBar";

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
    drawerOpen,
    onToggleDrawer,
    pendingCount,
    canApply,
    onApplyClick,
    onExportClick,
  }: {
    snapshots: SnapshotSummary[];
    baselineId: number | null;
    comparisonId: number | null;
    comparison: ComparisonSummary | null;
    pair: PairStats | null;
    same: boolean;
    onSelectBaseline: (id: number) => void;
    onSelectComparison: (id: number) => void;
    onSwap: () => void;
    drawerOpen: boolean;
    onToggleDrawer: () => void;
    pendingCount: number;
    canApply: boolean;
    onApplyClick: () => void;
    onExportClick: () => void;
  } = $props();

  let seqs = $derived(snapshotSeqs(snapshots));
  let options = $derived(selectorOptions(snapshots, seqs));
  let baseline = $derived(snapshots.find((s) => s.id === baselineId));
  let comparisonSnapshot = $derived(snapshots.find((s) => s.id === comparisonId));
  let note = $derived(
    baseline && comparisonSnapshot && comparison && pair
      ? spanNote(baseline, comparisonSnapshot, comparison, pair, same)
      : "",
  );
  let chips = $derived(pair ? deltaChips(pair) : []);

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
  {#if baseline && comparisonSnapshot}
    <div class="selectors">
      <div class="selector-panel">
        <span class="selector-label">baseline</span>
        <select class="selector" value={baselineId} onchange={handleBaselineChange}>
          {#each options as option (option.id)}
            <option value={option.id}>{option.label}</option>
          {/each}
        </select>
      </div>
      <button type="button" class="swap" onclick={onSwap} aria-label="Swap baseline and comparison">⇄</button>
      <div class="selector-panel">
        <span class="selector-label">comparison</span>
        <select class="selector" value={comparisonId} onchange={handleComparisonChange}>
          {#each options as option (option.id)}
            <option value={option.id}>{option.label}</option>
          {/each}
        </select>
      </div>
    </div>
  {/if}
  <span class="span-note">{note}</span>
  {#if pair}
    <div class="delta-chips">
      {#each chips as chip (chip.key)}
        <span class="chip chip--{chip.tone}">
          {#if chip.tone === "none"}
            {chip.label}
          {:else}
            <span class="chip-value">{chip.value}</span>
            <span class="chip-label">{chip.label}</span>
          {/if}
        </span>
      {/each}
    </div>
  {/if}
  {#if canApply}
    <button type="button" class="apply-btn" onclick={onApplyClick}>
      Apply {pendingCount} changes
    </button>
  {/if}
  {#if comparisonSnapshot}
    <button type="button" class="export-btn" onclick={onExportClick}>
      <span class="export-icon">⇩</span>Export
    </button>
  {/if}
  <button
    type="button"
    class="drawer-toggle"
    aria-expanded={drawerOpen}
    aria-label={drawerOpen ? "Close settings drawer" : "Open settings drawer"}
    onclick={onToggleDrawer}
  >
    ⚙
  </button>
</header>

<style>
  .head-bar {
    display: flex;
    align-items: center;
    gap: var(--s-9);
    height: var(--bar-head);
    flex: none;
    padding: 0 var(--s-8);
    border-bottom: 1px solid var(--rule);
    background: var(--surface);
  }

  .wordmark {
    display: flex;
    flex-direction: column;
    flex: none;
    gap: var(--s-1);
  }
  .brand {
    font-family: var(--font-mono);
    font-weight: 600;
    font-size: var(--t-head);
    letter-spacing: -0.02em;
    color: var(--ink);
  }
  .eyebrow {
    font-family: var(--font-sans);
    font-weight: 500;
    font-size: var(--t-tag);
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--ink-3);
  }

  .selectors {
    display: flex;
    align-items: center;
    flex: none;
    gap: var(--s-3);
  }
  .selector-panel {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 150px;
    padding: var(--s-3) var(--s-6);
    border: 1px solid var(--control-edge);
    border-radius: var(--r-selector);
    background: var(--surface-control-alt);
    cursor: pointer;
  }
  .selector-panel:focus-within {
    border-color: var(--ink);
    background: var(--surface);
  }
  .selector-label {
    font-family: var(--font-sans);
    font-weight: 500;
    font-size: var(--t-label-sm);
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--ink-3);
  }
  .selector {
    display: block;
    width: 100%;
    padding: 0;
    border: none;
    border-radius: 0;
    background: none;
    font-family: var(--font-mono);
    font-size: var(--t-body);
    color: var(--ink);
    cursor: pointer;
  }
  .selector:focus {
    outline: none;
  }
  .selector-panel:focus-within .selector {
    font-weight: 500;
  }
  .swap {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border: 1px solid var(--control-edge);
    border-radius: var(--r-selector);
    background: var(--surface);
    color: var(--ink-3);
    font-family: var(--font-sans);
    font-size: var(--t-meta);
    cursor: pointer;
  }

  .span-note {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--t-body-s);
    color: var(--ink-4);
  }

  .delta-chips {
    display: flex;
    align-items: center;
    flex: none;
    gap: var(--s-4);
    margin-left: auto;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: var(--s-3);
    height: var(--h-control);
    padding: 0 var(--s-5);
    border-radius: var(--r-chip);
    font-family: var(--font-mono);
    font-weight: 600;
    font-size: var(--t-body);
    white-space: nowrap;
  }
  .chip-label {
    font-family: var(--font-sans);
    font-size: 11px;
    font-weight: 500;
  }
  .chip--added {
    background: var(--state-added-bg);
    color: var(--state-added);
  }
  .chip--revoked {
    background: var(--state-revoked-bg);
    color: var(--state-revoked);
  }
  .chip--changed {
    background: var(--state-changed-bg);
    color: var(--state-changed);
  }
  .chip--escalation {
    border: 1px solid var(--escalation-edge);
    background: var(--escalation-bg);
    color: var(--escalation-ink);
  }
  .chip--none {
    color: var(--ink-3);
    font-weight: 400;
  }

  .export-btn {
    display: inline-flex;
    align-items: center;
    gap: var(--s-3);
    flex: none;
    height: var(--h-control);
    padding: 0 var(--s-6);
    border: 1px solid var(--ink);
    border-radius: var(--r-chip);
    background: var(--ink);
    font-family: var(--font-sans);
    font-weight: 600;
    font-size: var(--t-body-s);
    color: var(--surface);
    cursor: pointer;
  }
  .export-icon {
    font-size: var(--t-meta);
  }
  .apply-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    height: var(--h-control);
    margin-left: auto;
    padding: 0 var(--s-6);
    border: 1px solid var(--ink);
    border-radius: var(--r-chip);
    background: var(--ink);
    font-family: var(--font-sans);
    font-weight: 600;
    font-size: var(--t-body-s);
    color: var(--surface);
    cursor: pointer;
  }
  .drawer-toggle {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: 28px;
    height: 28px;
    border: 1px solid var(--control-edge);
    border-radius: var(--r-card);
    background: var(--surface);
    color: var(--ink-3);
    font-size: var(--t-body);
    line-height: 1;
    cursor: pointer;
  }

  /* Reflow order (ADR-0009): chip captions shed before anything else in the head bar. */
  @media (max-width: 1000px) {
    .delta-chips .chip-label {
      display: none;
    }
  }
</style>
