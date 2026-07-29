<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  let username = $state("");
  let appPassword = $state("");
  let workspace = $state("");
  let credentialsSaved = $state(false);

  let running = $state(false);
  let runResult = $state("");

  async function saveCredentials(event: Event) {
    event.preventDefault();
    await invoke("set_credentials", {
      username,
      appPassword,
      workspace,
    });
    credentialsSaved = true;
    runResult = "";
  }

  async function runNow() {
    running = true;
    runResult = "";
    try {
      const snapshotId = await invoke<number>("run_now");
      runResult = `Run complete — snapshot #${snapshotId} saved.`;
    } catch (err) {
      runResult = `Run failed: ${err}`;
    } finally {
      running = false;
    }
  }
</script>

<main class="container">
  <h1>repo-dashboard</h1>

  <section>
    <h2>Bitbucket credentials</h2>
    <form onsubmit={saveCredentials}>
      <input placeholder="Username" bind:value={username} required />
      <input placeholder="App password" type="password" bind:value={appPassword} required />
      <input placeholder="Workspace" bind:value={workspace} required />
      <button type="submit">Save credentials</button>
    </form>
    {#if credentialsSaved}
      <p>Credentials saved for this session.</p>
    {/if}
  </section>

  <section>
    <h2>Run</h2>
    <button onclick={runNow} disabled={!credentialsSaved || running}>
      {running ? "Running…" : "Run now"}
    </button>
    {#if runResult}
      <p>{runResult}</p>
    {/if}
  </section>

  <section>
    <a href="/dashboard">View roster dashboard →</a>
  </section>
</main>

<style>
  .container {
    margin: 0 auto;
    max-width: 480px;
    padding: 2rem 1rem;
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
  }

  section {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  input,
  button {
    padding: 0.5rem 0.75rem;
    font-size: 1rem;
  }
</style>
