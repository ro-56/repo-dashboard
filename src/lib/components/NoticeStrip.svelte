<script lang="ts">
  import type { RosterTree } from "$lib/roster";
  import { structuralNotice } from "$lib/noticeStrip";

  let {
    tree,
    same,
    comparisonSeq,
  }: {
    tree: RosterTree;
    same: boolean;
    comparisonSeq: number;
  } = $props();

  let notice = $derived(structuralNotice(tree, same, comparisonSeq));
</script>

{#if notice}
  <div class="notice-strip">
    <span class="eyebrow">structural</span>
    <span class="prose">{notice}</span>
  </div>
{/if}

<style>
  .notice-strip {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: var(--s-5);
    flex: none;
    padding: var(--s-3) var(--s-8);
    background: var(--notice-bg);
    border-bottom: 1px solid var(--rule);
    box-shadow: inset 3px 0 0 var(--notice-edge);
    overflow: hidden;
  }

  .eyebrow {
    flex: none;
    font-family: var(--font-sans);
    font-weight: 500;
    font-size: var(--t-tag);
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--notice-ink);
  }

  .prose {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-sans);
    font-size: var(--t-body);
    color: var(--notice-ink);
  }
</style>
