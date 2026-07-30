<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { goto } from "$app/navigation";

  let running = $state(false);
  let error = $state("");

  // ADR-0010: a run always jumps Baseline/Comparison to the new latest-vs-previous pair, even
  // over a manually pinned selection — dropping the baseline/comparison query params and
  // forcing a reload lets +page.ts's load fall back to its own latest/previous defaults.
  async function runNow() {
    running = true;
    error = "";
    try {
      await invoke("run_now");
      await goto("/dashboard", { invalidateAll: true, keepFocus: true, noScroll: true });
    } catch (err) {
      error = String(err);
    } finally {
      running = false;
    }
  }
</script>

<section class="run-panel">
  <h3 class="panel-title">run</h3>
  <button type="button" onclick={runNow} disabled={running}>
    {running ? "Running…" : "Get all data"}
  </button>
  {#if error}
    <p class="error">{error}</p>
  {/if}
</section>

<style>
  .run-panel {
    display: flex;
    flex-direction: column;
    gap: var(--s-6);
    margin-top: var(--s-8);
    padding-top: var(--s-8);
    border-top: 1px solid var(--rule);
  }

  .panel-title {
    margin: 0;
    font-family: var(--font-sans);
    font-weight: 600;
    font-size: var(--t-body);
    color: var(--ink);
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
