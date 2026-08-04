<script lang="ts">
  import { _ } from "svelte-i18n";
  import type { RosterTree } from "$lib/roster";
  import { excludedBannerCount, excludedPrincipals } from "$lib/excludedBanner";

  let {
    tree,
    excludedIds,
    onUnhide,
    onUnhideAll,
  }: {
    tree: RosterTree;
    excludedIds: ReadonlySet<string>;
    onUnhide: (id: string) => void;
    onUnhideAll: () => void;
  } = $props();

  let list = $derived(excludedPrincipals(tree, excludedIds));
  let countText = $derived(excludedBannerCount($_, excludedIds));

  // Local to this banner — collapsing/expanding the manage list isn't part of any other
  // component's state, unlike toggledRepos/viewMode/searchQuery which +page.svelte owns.
  let manageOpen = $state(false);
  function toggleManage() {
    manageOpen = !manageOpen;
  }
</script>

<div class="banner">
  <span class="count">{countText}</span>
  <button type="button" class="manage" onclick={toggleManage}>
    {manageOpen ? $_("excludedBanner.hideList") : $_("excludedBanner.manage")}
  </button>
</div>
{#if manageOpen}
  <div class="list">
    {#each list as p (p.id)}
      <div class="item">
        <span class="label">{p.label}</span>
        <button type="button" class="unhide" onclick={() => onUnhide(p.id)}>{$_("excludedBanner.unhide")}</button>
      </div>
    {/each}
    <button type="button" class="unhide-all" onclick={onUnhideAll}>{$_("excludedBanner.unhideAll")}</button>
  </div>
{/if}

<style>
  .banner {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: var(--s-4);
    flex: none;
    padding: var(--s-3) var(--s-8);
    border-bottom: 1px solid var(--rule);
    background: var(--surface-sunken);
  }
  .count {
    font-family: var(--font-sans);
    font-size: var(--t-meta);
    color: var(--ink-3);
  }
  .manage {
    margin-left: auto;
    border: none;
    padding: 0;
    background: none;
    font-family: var(--font-sans);
    font-weight: 600;
    font-size: var(--t-meta);
    color: var(--state-changed);
    cursor: pointer;
  }

  .list {
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    gap: var(--s-4);
    flex: none;
    padding: var(--s-5) var(--s-8);
    border-bottom: 1px solid var(--rule);
    background: var(--surface-raised);
  }
  .item {
    display: flex;
    align-items: center;
    gap: var(--s-4);
  }
  .label {
    font-family: var(--font-sans);
    font-size: var(--t-body-s);
    color: var(--ink);
  }
  .unhide {
    margin-left: auto;
    flex: none;
    border: none;
    padding: 0;
    background: none;
    font-family: var(--font-sans);
    font-size: var(--t-tag);
    color: var(--state-changed);
    cursor: pointer;
  }
  .unhide-all {
    align-self: flex-start;
    border: none;
    padding: 0;
    background: none;
    font-family: var(--font-sans);
    font-size: var(--t-tag);
    color: var(--state-revoked);
    cursor: pointer;
  }
</style>
