<script lang="ts">
  import type { PageData } from "./$types";

  let { data }: { data: PageData } = $props();
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
            <article class="repo-card">
              <h3>{repo.repo}</h3>
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
    padding: 0.75rem 1rem;
  }

  .repo-card h3 {
    margin: 0 0 0.5rem;
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
</style>
