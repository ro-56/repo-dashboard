<script lang="ts">
  import { SvelteSet } from "svelte/reactivity";
  import type { PageData } from "./$types";
  import ProjectSection from "$lib/components/ProjectSection.svelte";
  import RosterColumnLabels from "$lib/components/RosterColumnLabels.svelte";
  import { treeHasAnyChanges } from "$lib/repoCard";

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
</script>

<main class="dashboard">
  <h1>Roster dashboard</h1>

  {#if data.snapshots.length === 0}
    <p>No runs recorded yet — use "Run now" on the home page first.</p>
  {:else}
    <p class="run-pair">
      Run pair: Snapshot #{data.snapshotAId} → Snapshot #{data.snapshotBId}
    </p>

    <RosterColumnLabels />
    <div class="canvas">
      {#each data.tree as project (project.repoProject)}
        <ProjectSection
          {project}
          snapshotBId={data.snapshotBId}
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

  h1 {
    margin: var(--s-7) var(--s-7) 0;
    font-size: var(--t-head);
  }

  .run-pair {
    margin: var(--s-3) var(--s-7) var(--s-7);
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
