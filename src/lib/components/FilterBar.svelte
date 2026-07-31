<script lang="ts">
  import type { ViewMode } from "$lib/filterBar";

  let {
    viewMode,
    countNoteText,
    expandLabel,
    bulkDisabled,
    onSetViewMode,
    onBulkToggle,
    onReset,
  }: {
    viewMode: ViewMode;
    countNoteText: string;
    expandLabel: string;
    bulkDisabled: boolean;
    onSetViewMode: (mode: ViewMode) => void;
    onBulkToggle: () => void;
    onReset: () => void;
  } = $props();
</script>

<div class="filter-bar">
  <div class="tabs">
    <button type="button" class="tab" class:active={viewMode === "all"} onclick={() => onSetViewMode("all")}>
      All access
    </button>
    <button
      type="button"
      class="tab"
      class:active={viewMode === "changes"}
      onclick={() => onSetViewMode("changes")}
    >
      Changes only
    </button>
  </div>
  <div class="right-controls">
    <span class="count-note">{countNoteText}</span>
    <button type="button" class="control" disabled={bulkDisabled} onclick={onBulkToggle}>{expandLabel}</button>
    <button type="button" class="control" onclick={onReset}>Reset</button>
  </div>
</div>

<style>
  .filter-bar {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: var(--s-5);
    height: var(--bar-filters);
    flex: none;
    padding: 0 var(--s-8);
    border-bottom: 1px solid var(--rule);
    background: var(--surface-sunken);
  }

  .tabs {
    display: flex;
    flex: none;
    padding: 2px;
    border-radius: var(--r-card);
    background: var(--track);
  }
  .tab {
    display: inline-flex;
    align-items: center;
    flex: none;
    height: 28px;
    white-space: nowrap;
    padding: 0 var(--s-7);
    border: none;
    border-radius: var(--r-row);
    background: none;
    font-family: var(--font-sans);
    font-weight: 500;
    font-size: var(--t-body-s);
    color: var(--ink-4);
    cursor: pointer;
  }
  .tab.active {
    background: var(--track-active);
    box-shadow: var(--shadow-segment);
    font-weight: 600;
    color: var(--ink);
  }

  .right-controls {
    display: flex;
    align-items: center;
    gap: var(--s-5);
    flex: none;
    margin-left: auto;
  }
  .count-note {
    font-family: var(--font-mono);
    font-size: var(--t-body-s);
    color: var(--ink-mute);
    white-space: nowrap;
  }
  .control {
    display: inline-flex;
    align-items: center;
    flex: none;
    height: var(--h-control);
    white-space: nowrap;
    padding: 0 var(--s-6);
    border: 1px solid var(--control-edge);
    border-radius: var(--r-card);
    background: var(--surface);
    font-family: var(--font-sans);
    font-weight: 500;
    font-size: var(--t-body-s);
    color: var(--ink-3);
    cursor: pointer;
  }
  .control:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
