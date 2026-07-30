<script lang="ts">
  import type { ProjectNode } from "$lib/roster";
  import { countBadges, projectBreakdown, projectMeta } from "$lib/repoCard";
  import { visibleRepos, type ViewMode } from "$lib/filterBar";
  import CountBadges from "./CountBadges.svelte";
  import RepoCard from "./RepoCard.svelte";

  let {
    project,
    comparisonId,
    anyChanges,
    viewMode,
    isToggled,
    onToggle,
  }: {
    project: ProjectNode;
    comparisonId: number;
    anyChanges: boolean;
    viewMode: ViewMode;
    isToggled: (repoProject: string, repo: string) => boolean;
    onToggle: (repoProject: string, repo: string) => void;
  } = $props();

  // Meta/counts always describe the whole project (ADR-0008) — only the rendered repo list
  // narrows under Changes only.
  let counts = $derived(countBadges(projectBreakdown(project)));
  let meta = $derived(projectMeta(project));
  let repos = $derived(visibleRepos(project, viewMode));
</script>

{#if repos.length > 0}
  <section class="project">
    <header class="project-head">
      <span class="project-name">{project.repoProject}</span>
      <span class="project-meta">{meta}</span>
      <span class="project-counts"><CountBadges badges={counts} /></span>
    </header>
    <div class="repo-cards">
      {#each repos as repo (repo.repo)}
        <RepoCard
          {repo}
          {comparisonId}
          {anyChanges}
          {viewMode}
          toggled={isToggled(project.repoProject, repo.repo)}
          onToggle={() => onToggle(project.repoProject, repo.repo)}
        />
      {/each}
    </div>
  </section>
{/if}

<style>
  .project {
    margin-top: var(--s-7);
  }
  .project:first-child {
    margin-top: 0;
  }

  .project-head {
    display: flex;
    align-items: center;
    gap: var(--s-7);
    padding: 0 var(--s-1) var(--s-2);
    border-bottom: 2px solid var(--ink);
  }

  .project-name {
    font-family: var(--font-mono);
    font-weight: 600;
    font-size: var(--t-id);
    letter-spacing: -0.01em;
    color: var(--ink);
  }

  .project-meta {
    font-family: var(--font-sans);
    font-size: var(--t-meta);
    color: var(--ink-mute);
  }

  .project-counts {
    margin-left: auto;
  }

  .repo-cards {
    margin-top: var(--s-2);
    display: flex;
    flex-direction: column;
    gap: var(--s-2);
  }
</style>
