<script lang="ts">
  import { _ } from "svelte-i18n";
  import type { ProjectNode } from "$lib/roster";
  import { countBadges, projectBreakdown, projectMeta } from "$lib/repoCard";
  import { visibleRepos, type ViewMode } from "$lib/filterBar";
  import type { PendingEdits, StagedEdit } from "$lib/pendingEdits";
  import CountBadges from "./CountBadges.svelte";
  import RepoCard from "./RepoCard.svelte";

  let {
    project,
    comparisonId,
    anyChanges,
    viewMode,
    searchQuery,
    isToggled,
    onToggle,
    pending,
    editingEnabled,
    onStage,
    onUndo,
  }: {
    project: ProjectNode;
    comparisonId: number;
    anyChanges: boolean;
    viewMode: ViewMode;
    searchQuery: string;
    isToggled: (repoProject: string, repo: string) => boolean;
    onToggle: (repoProject: string, repo: string) => void;
    pending: PendingEdits;
    editingEnabled: boolean;
    onStage: (edit: StagedEdit) => void;
    onUndo: (key: string) => void;
  } = $props();

  // Meta/counts always describe the whole project (ADR-0008) — only the rendered repo list
  // narrows under Changes only / an active search.
  let counts = $derived(countBadges($_, projectBreakdown(project)));
  let meta = $derived(projectMeta($_, project));
  let repos = $derived(visibleRepos(project, viewMode, searchQuery));
  // Every repo's principals across the whole Project, unfiltered by `viewMode` (PD-62) — the
  // cascade-count candidate pool for a Project-scope Group grant needs every Member it derives,
  // not just whichever repos "Changes only" happens to be showing right now.
  let projectPrincipals = $derived(project.repos.flatMap((r) => r.principals));
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
          {searchQuery}
          toggled={isToggled(project.repoProject, repo.repo)}
          onToggle={() => onToggle(project.repoProject, repo.repo)}
          {projectPrincipals}
          {pending}
          {editingEnabled}
          {onStage}
          {onUndo}
        />
      {/each}
    </div>
  </section>
{/if}

<style>
  .project {
    margin-bottom: var(--s-9);
  }

  .project-head {
    display: flex;
    align-items: center;
    gap: var(--s-6);
    padding: 0 var(--s-2) var(--s-4);
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
    display: flex;
    flex-direction: column;
    gap: var(--s-4);
  }

  /* Reflow order (ADR-0009): project meta sheds after the repo card's grants text and bar. */
  @media (max-width: 1100px) {
    .project-meta {
      display: none;
    }
  }
</style>
