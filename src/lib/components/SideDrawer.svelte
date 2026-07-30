<script lang="ts">
  import type { Snippet } from "svelte";

  // Persistent slide-out panel (ADR-0010) — not a modal, so there is no backdrop and no
  // click-outside dismissal. Content stays mounted while closed (rather than an {#if}) so
  // sub-panel state (e.g. an in-progress credentials edit) survives a close/reopen.
  let { open, onClose, children }: { open: boolean; onClose: () => void; children: Snippet } = $props();
</script>

<aside class="drawer" class:open aria-hidden={!open} inert={!open}>
  <div class="drawer-head">
    <span class="drawer-title">settings</span>
    <button type="button" class="close" onclick={onClose} aria-label="Close settings drawer">×</button>
  </div>
  <div class="drawer-body">
    {@render children()}
  </div>
</aside>

<style>
  .drawer {
    position: fixed;
    top: var(--bar-head);
    right: 0;
    bottom: 0;
    display: flex;
    flex-direction: column;
    width: 300px;
    background: var(--surface-raised);
    border-left: 1px solid var(--rule);
    transform: translateX(100%);
    transition: transform 160ms ease;
    z-index: 10;
  }
  .drawer.open {
    transform: translateX(0);
  }

  .drawer-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex: none;
    height: var(--bar-filters);
    padding: 0 var(--s-7);
    border-bottom: 1px solid var(--rule);
  }
  .drawer-title {
    font-family: var(--font-sans);
    font-weight: 500;
    font-size: var(--t-tag);
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--ink-mute);
  }
  .close {
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
    font-size: var(--t-body);
    line-height: 1;
    cursor: pointer;
  }

  .drawer-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: var(--s-8) var(--s-7);
  }
</style>
