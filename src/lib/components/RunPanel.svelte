<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { goto } from "$app/navigation";
  import { _ } from "svelte-i18n";

  type RunProgress = { index: number; total: number; repo: string };

  let running = $state(false);
  let error = $state("");
  let progress = $state<RunProgress | null>(null);
  let unlisten: UnlistenFn | undefined;

  // Catches the case where this component unmounts (e.g. navigation away) mid-Run, since the
  // runNow() finally block never gets to run in that case.
  $effect(() => {
    return () => unlisten?.();
  });

  // ADR-0010: a run always jumps Baseline/Comparison to the new latest-vs-previous pair, even
  // over a manually pinned selection — dropping the baseline/comparison query params and
  // forcing a reload lets +page.ts's load fall back to its own latest/previous defaults.
  async function runNow() {
    running = true;
    error = "";
    progress = null;
    try {
      unlisten = await listen<RunProgress>("run-progress", (event) => {
        progress = event.payload;
      });
      await invoke("run_now");
      await goto("/dashboard", { invalidateAll: true, keepFocus: true, noScroll: true });
    } catch (err) {
      error = String(err);
    } finally {
      unlisten?.();
      unlisten = undefined;
      running = false;
      progress = null;
    }
  }
</script>

<section class="run-panel">
  <h3 class="panel-title">{$_("runPanel.title")}</h3>
  {#if running && progress}
    <p class="progress-text" aria-live="polite">
      {$_("runPanel.fetching", { values: { repo: progress.repo, index: progress.index, total: progress.total } })}
    </p>
    <div class="progress-track" role="progressbar" aria-valuenow={progress.index} aria-valuemin={0} aria-valuemax={progress.total}>
      <div class="progress-fill" style:width="{(progress.index / progress.total) * 100}%"></div>
    </div>
  {:else}
    <button type="button" onclick={runNow} disabled={running}>
      {running ? $_("runPanel.running") : $_("runPanel.getAllData")}
    </button>
  {/if}
  {#if error}
    <p class="error">{error}</p>
  {/if}
</section>

<style>
  .run-panel {
    display: flex;
    flex-direction: column;
    gap: var(--s-5);
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
    border-radius: var(--r-row);
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

  .progress-text {
    margin: 0;
    font-family: var(--font-sans);
    font-size: var(--t-body-s);
    color: var(--ink-mute);
  }

  .progress-track {
    width: 100%;
    height: 4px;
    border-radius: 2px;
    background: var(--track);
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    border-radius: 2px;
    background: var(--ink);
    transition: width 150ms ease-out;
  }
</style>
