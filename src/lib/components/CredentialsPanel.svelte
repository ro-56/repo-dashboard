<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { CredentialsSummary } from "$lib/credentials";

  let loading = $state(true);
  let summary = $state<CredentialsSummary | null>(null);
  // Edit is the default until the first get_credentials response lands, so a fresh install
  // (nothing in the keychain yet) opens straight into the form instead of a flash of "idle".
  let mode = $state<"idle" | "edit">("edit");

  let username = $state("");
  let appPassword = $state("");
  let workspace = $state("");
  let submitting = $state(false);
  let error = $state("");

  async function loadCredentials() {
    loading = true;
    try {
      summary = await invoke<CredentialsSummary>("get_credentials");
      if (summary.hasCredentials) {
        mode = "idle";
        username = summary.username ?? "";
        workspace = summary.workspace ?? "";
      }
    } catch (err) {
      error = String(err);
    } finally {
      loading = false;
    }
  }

  onMount(loadCredentials);

  function startEdit() {
    error = "";
    mode = "edit";
  }

  function cancelEdit() {
    error = "";
    appPassword = "";
    username = summary?.username ?? "";
    workspace = summary?.workspace ?? "";
    mode = "idle";
  }

  async function handleSubmit(event: Event) {
    event.preventDefault();
    submitting = true;
    error = "";
    try {
      await invoke("set_credentials", { username, appPassword, workspace });
      summary = { username, workspace, hasCredentials: true };
      appPassword = "";
      mode = "idle";
    } catch (err) {
      error = String(err);
    } finally {
      submitting = false;
    }
  }
</script>

<section class="credentials-panel">
  <h3 class="panel-title">credentials</h3>

  {#if loading}
    <p class="status">loading…</p>
  {:else if mode === "idle" && summary}
    <p class="connected">
      connected as <code>{summary.username}</code> on <code>{summary.workspace}</code>
    </p>
    <button type="button" class="secondary" onclick={startEdit}>Change credentials</button>
  {:else}
    <form onsubmit={handleSubmit}>
      <input aria-label="Username" placeholder="Username" bind:value={username} required />
      <input
        aria-label="App password"
        placeholder="App password"
        type="password"
        bind:value={appPassword}
        required
      />
      <input aria-label="Workspace" placeholder="Workspace" bind:value={workspace} required />
      <div class="actions">
        <button type="submit" disabled={submitting}>{submitting ? "Saving…" : "Save credentials"}</button>
        {#if summary?.hasCredentials}
          <button type="button" class="secondary" onclick={cancelEdit} disabled={submitting}>Cancel</button>
        {/if}
      </div>
    </form>
    {#if error}
      <p class="error">{error}</p>
    {/if}
  {/if}
</section>

<style>
  .credentials-panel {
    display: flex;
    flex-direction: column;
    gap: var(--s-6);
  }

  .panel-title {
    margin: 0;
    font-family: var(--font-sans);
    font-weight: 600;
    font-size: var(--t-body);
    color: var(--ink);
  }

  .status {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--t-body-s);
    color: var(--ink-mute);
  }

  .connected {
    margin: 0;
    font-family: var(--font-sans);
    font-size: var(--t-body);
    line-height: 1.5;
    color: var(--ink-2);
  }
  .connected code {
    font-family: var(--font-mono);
    font-size: var(--t-body-s);
    color: var(--ink);
  }

  form {
    display: flex;
    flex-direction: column;
    gap: var(--s-5);
  }

  input {
    height: 26px;
    padding: 0 var(--s-6);
    border: 1px solid var(--control-edge);
    border-radius: var(--radius);
    background: var(--surface);
    font-family: var(--font-mono);
    font-size: var(--t-body);
    color: var(--ink);
  }

  .actions {
    display: flex;
    gap: var(--s-5);
  }

  button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: 24px;
    padding: 0 var(--s-7);
    border: 1px solid var(--control-edge);
    border-radius: var(--radius);
    background: var(--surface-sunken);
    font-family: var(--font-sans);
    font-weight: 500;
    font-size: var(--t-body);
    color: var(--ink);
    cursor: pointer;
  }
  button.secondary {
    background: var(--surface);
    color: var(--ink-3);
  }
  button:disabled {
    cursor: default;
    color: var(--ink-mute);
  }

  .error {
    margin: 0;
    font-family: var(--font-sans);
    font-size: var(--t-body-s);
    color: var(--state-revoked);
  }
</style>
