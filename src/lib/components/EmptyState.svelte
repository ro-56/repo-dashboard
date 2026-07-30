<script lang="ts">
  import type { EmptyStateView } from "$lib/emptyState";

  let { state, onAction }: { state: EmptyStateView; onAction: () => void } = $props();
</script>

<div class="empty-wrap">
  <div class="empty-card">
    <span class="title">{state.title}</span>
    <span class="body">{state.body}</span>
    <button type="button" class="action" onclick={onAction}>{state.actionLabel}</button>
  </div>
</div>

<style>
  .empty-wrap {
    margin: 56px auto 0;
    max-width: 520px;
    text-align: center;
  }
  .empty-card {
    display: inline-flex;
    flex-direction: column;
    align-items: center;
    gap: var(--s-6);
    padding: 20px 26px;
    background: var(--surface);
    border: 1px solid var(--rule);
    border-radius: var(--radius);
  }
  .title {
    font-family: var(--font-sans);
    font-weight: 600;
    font-size: var(--t-head);
    line-height: 1.3;
    color: var(--ink);
  }
  .body {
    max-width: 380px;
    font-family: var(--font-sans);
    font-size: var(--t-body);
    line-height: 1.5;
    color: var(--ink-3);
    text-wrap: pretty;
  }
  .action {
    display: inline-flex;
    align-items: center;
    height: 22px;
    margin-top: var(--s-1);
    padding: 0 var(--s-7);
    border: 1px solid var(--control-edge);
    border-radius: var(--radius);
    background: var(--surface-sunken);
    font-family: var(--font-sans);
    font-weight: 500;
    font-size: var(--t-body);
    color: var(--ink);
    cursor: pointer;
  }
</style>
