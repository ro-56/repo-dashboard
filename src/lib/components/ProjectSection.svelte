<script lang="ts">
  import type { ProjectNode } from "$lib/roster";
  import { countBadges, projectBreakdown, projectMeta } from "$lib/repoCard";
  import CountBadges from "./CountBadges.svelte";
  import RepoCard from "./RepoCard.svelte";

  let {
    project,
    snapshotBId,
    anyChanges,
    isToggled,
    onToggle,
  }: {
    project: ProjectNode;
    snapshotBId: number;
    anyChanges: boolean;
    isToggled: (repoProject: string, repo: string) => boolean;
    onToggle: (repoProject: string, repo: string) => void;
  } = $props();

  let counts = $derived(countBadges(projectBreakdown(project)));
  let meta = $derived(projectMeta(project));
</script>

<section class="project">
  <header class="project-head">
    <span class="project-name">{project.repoProject}</span>
    <span class="project-meta">{meta}</span>
    <span class="project-counts"><CountBadges badges={counts} /></span>
  </header>
  <div class="repo-cards">
    {#each project.repos as repo (repo.repo)}
      <RepoCard
        {repo}
        {snapshotBId}
        {anyChanges}
        toggled={isToggled(project.repoProject, repo.repo)}
        onToggle={() => onToggle(project.repoProject, repo.repo)}
      />
    {/each}
  </div>
</section>

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
