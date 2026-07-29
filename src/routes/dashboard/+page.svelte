<script lang="ts">
  import { SvelteSet } from "svelte/reactivity";
  import { goto } from "$app/navigation";
  import type { PageData } from "./$types";
  import HeadBar from "$lib/components/HeadBar.svelte";
  import SummaryBar from "$lib/components/SummaryBar.svelte";
  import NoticeStrip from "$lib/components/NoticeStrip.svelte";
  import ProjectSection from "$lib/components/ProjectSection.svelte";
  import RosterColumnLabels from "$lib/components/RosterColumnLabels.svelte";
  import { treeHasAnyChanges } from "$lib/repoCard";
  import { snapshotSeqs } from "$lib/headBar";

  let { data }: { data: PageData } = $props();

  // Keys of repos the user has clicked away from their computed default-open state
  // (PD-16's default-expand rule) — not "which repos are open" directly, so that rule keeps
  // applying to every repo the user hasn't touched even as the underlying tree changes.
  let toggledRepos = new SvelteSet<string>();

  function repoKey(repoProject: string, repo: string): string {
    return `${repoProject}::${repo}`;
  }

  function toggleRepo(repoProject: string, repo: string) {
    const key = repoKey(repoProject, repo);
    if (toggledRepos.has(key)) {
      toggledRepos.delete(key);
    } else {
      toggledRepos.add(key);
    }
  }

  function isToggled(repoProject: string, repo: string): boolean {
    return toggledRepos.has(repoKey(repoProject, repo));
  }

  let anyChanges = $derived(treeHasAnyChanges(data.tree));
  let seqs = $derived(snapshotSeqs(data.snapshots));
  let same = $derived(data.baselineId === data.comparisonId);

  function navigateToPair(baselineId: number, comparisonId: number) {
    const params = new URLSearchParams({ baseline: String(baselineId), comparison: String(comparisonId) });
    goto(`?${params.toString()}`, { keepFocus: true, noScroll: true });
  }

  function selectBaseline(id: number) {
    navigateToPair(id, data.comparisonId);
  }
  function selectComparison(id: number) {
    navigateToPair(data.baselineId, id);
  }
  function swapPair() {
    navigateToPair(data.comparisonId, data.baselineId);
  }
</script>

<main class="dashboard">
  {#if data.snapshots.length === 0}
    <p class="empty-note">No runs recorded yet — use "Run now" on the home page first.</p>
  {:else}
    <HeadBar
      snapshots={data.snapshots}
      baselineId={data.baselineId}
      comparisonId={data.comparisonId}
      comparison={data.comparison!}
      pair={data.pair!}
      {same}
      onSelectBaseline={selectBaseline}
      onSelectComparison={selectComparison}
      onSwap={swapPair}
    />
    <SummaryBar
      comparison={data.comparison!}
      pair={data.pair!}
      {same}
      baselineSeq={seqs.get(data.baselineId)!}
      comparisonSeq={seqs.get(data.comparisonId)!}
    />
    <NoticeStrip tree={data.tree} {same} comparisonSeq={seqs.get(data.comparisonId)!} />

    <RosterColumnLabels />
    <div class="canvas">
      {#each data.tree as project (project.repoProject)}
        <ProjectSection
          {project}
          comparisonId={data.comparisonId}
          {anyChanges}
          {isToggled}
          onToggle={toggleRepo}
        />
      {/each}
    </div>
  {/if}
</main>

<style>
  .dashboard {
    display: flex;
    flex-direction: column;
    font-family: var(--font-sans);
    color: var(--ink);
  }

  .empty-note {
    margin: var(--s-7);
    font-family: var(--font-mono);
    font-size: var(--t-body);
    color: var(--ink-3);
  }

  .canvas {
    flex: 1;
    background: var(--canvas);
    padding: var(--s-7) var(--s-7) var(--s-8);
  }
</style>
