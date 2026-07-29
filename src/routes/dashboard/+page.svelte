<script lang="ts">
  import { SvelteSet } from "svelte/reactivity";
  import type { PageData } from "./$types";
  import type { DiffStatus, PrincipalEntry, RepoNode } from "$lib/roster";

  let { data }: { data: PageData } = $props();

  let expandedRepos = new SvelteSet<string>();

  function repoKey(repoProject: string, repo: string): string {
    return `${repoProject}::${repo}`;
  }

  function toggleRepo(repoProject: string, repo: string) {
    const key = repoKey(repoProject, repo);
    if (expandedRepos.has(key)) {
      expandedRepos.delete(key);
    } else {
      expandedRepos.add(key);
    }
    console.log("expandedRepos:", Array.from(expandedRepos));
  }

  // A principal can appear more than once per repo (e.g. Direct plus Member-of-group-X),
  // so the key needs the access type — and, for Member, the group_id — to stay unique.
  function principalEntryKey(entry: PrincipalEntry): string {
    const groupId = entry.accessType.type === "Member" ? entry.accessType.group_id : "";
    return `${entry.principal.id}::${entry.accessType.type}::${groupId}`;
  }

  function diffLabel(status: DiffStatus): string | null {
    switch (status.status) {
      case "None":
        return null;
      case "Grant":
        return "Grant";
      case "Revoke":
        return "Revoke";
      case "LevelChange":
        return status.kind === "Escalation"
          ? `Escalation: ${status.from} → ${status.to}`
          : `Demotion: ${status.from} → ${status.to}`;
    }
  }

  function diffClass(status: DiffStatus): string {
    switch (status.status) {
      case "None":
        return "";
      case "Grant":
        return "diff-grant";
      case "Revoke":
        return "diff-revoke";
      case "LevelChange":
        return status.kind === "Escalation" ? "diff-escalation" : "diff-demotion";
    }
  }

  // A repo present in one compared Snapshot's discovery but absent from the other's
  // (one status is null, the other isn't) — distinct from a repo genuinely fetched on both
  // sides with zero grants, which must not be struck through.
  function absentSide(repo: RepoNode): "A" | "B" | null {
    if (repo.statusA === null && repo.statusB !== null) return "A";
    if (repo.statusB === null && repo.statusA !== null) return "B";
    return null;
  }
</script>

<main class="container">
  <h1>Roster dashboard</h1>

  {#if data.snapshots.length === 0}
    <p>No runs recorded yet — use "Run now" on the home page first.</p>
  {:else}
    <p class="run-pair">
      Run pair: Snapshot #{data.snapshotAId} → Snapshot #{data.snapshotBId}
    </p>

    {#each data.tree as project (project.repoProject)}
      <section class="project">
        <h2>{project.repoProject}</h2>
        <div class="repo-cards">
          {#each project.repos as repo (repo.repo)}
            {const isExpanded = $derived(expandedRepos.has(repoKey(repo.repoProject, repo.repo)))}
            {const absent = $derived(absentSide(repo))}
            <article class="repo-card" class:expanded={isExpanded} class:absent={absent !== null}>
              <button
                type="button"
                class="repo-card-header"
                aria-expanded={isExpanded}
                onclick={() => toggleRepo(repo.repoProject, repo.repo)}
              >
                <h3>{repo.repo}</h3>
                {#if absent}
                  <p class="absent-note">
                    Not discovered in Snapshot #{absent === "A" ? data.snapshotAId : data.snapshotBId}
                  </p>
                {/if}
                <dl class="counts">
                  <div class="count">
                    <dt>Read</dt>
                    <dd>{repo.readCount}</dd>
                  </div>
                  <div class="count">
                    <dt>Write</dt>
                    <dd>{repo.writeCount}</dd>
                  </div>
                  <div class="count">
                    <dt>Admin</dt>
                    <dd>{repo.adminCount}</dd>
                  </div>
                  <div class="count change-count">
                    <dt>Changes</dt>
                    <dd>{repo.changeCount}</dd>
                  </div>
                </dl>
              </button>

              {#if isExpanded}
                <ul class="roster">
                  {#each repo.principals as entry (principalEntryKey(entry))}
                    {const label = diffLabel(entry.diffStatus)}
                    <li class="roster-row">
                      <span class="roster-label">{entry.principal.label}</span>
                      <span class="roster-access">{entry.accessType.type}</span>
                      <span class="roster-permission">{entry.permission}</span>
                      {#if label}
                        <span class={["diff-marker", diffClass(entry.diffStatus)]}>{label}</span>
                      {/if}
                    </li>
                  {/each}
                </ul>
              {/if}
            </article>
          {/each}
        </div>
      </section>
    {/each}
  {/if}
</main>

<style>
  .container {
    margin: 0 auto;
    max-width: 960px;
    padding: 2rem 1rem;
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
  }

  .run-pair {
    color: #555;
  }

  .project {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .repo-cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 1rem;
  }

  .repo-card {
    border: 1px solid #ddd;
    border-radius: 0.5rem;
  }

  .repo-card-header {
    display: block;
    width: 100%;
    padding: 0.75rem 1rem;
    border: none;
    background: none;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .repo-card.expanded .repo-card-header {
    border-bottom: 1px solid #eee;
  }

  .repo-card h3 {
    margin: 0 0 0.5rem;
  }

  .repo-card.absent .repo-card-header,
  .repo-card.absent .roster {
    text-decoration: line-through;
    opacity: 0.55;
  }

  .absent-note {
    margin: -0.25rem 0 0.5rem;
    font-size: 0.75rem;
    font-weight: 600;
    text-decoration: none;
    color: #9e2b20;
  }

  .counts {
    display: flex;
    gap: 1rem;
    margin: 0;
  }

  .count {
    display: flex;
    flex-direction: column;
    align-items: center;
  }

  .count dt {
    font-size: 0.75rem;
    color: #777;
    text-transform: uppercase;
  }

  .count dd {
    margin: 0;
    font-size: 1.1rem;
    font-weight: 600;
  }

  .change-count dd {
    color: #b45309;
  }

  .roster {
    list-style: none;
    margin: 0;
    padding: 0.5rem 1rem 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .roster-row {
    display: flex;
    justify-content: space-between;
    gap: 0.75rem;
    font-size: 0.85rem;
  }

  .roster-label {
    flex: 1;
    font-weight: 500;
  }

  .roster-access {
    color: #777;
  }

  .roster-permission {
    text-transform: capitalize;
    font-weight: 600;
    min-width: 3.5rem;
    text-align: right;
  }

  .diff-marker {
    font-size: 0.7rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.02em;
    padding: 0.1rem 0.4rem;
    border-radius: 0.25rem;
    white-space: nowrap;
  }

  .diff-grant {
    color: #10653c;
    background: #e3f3ea;
  }

  .diff-revoke,
  .diff-escalation {
    color: #9e2b20;
    background: #f8ece9;
  }

  .diff-demotion {
    color: #4b46a8;
    background: #eceafc;
  }
</style>
