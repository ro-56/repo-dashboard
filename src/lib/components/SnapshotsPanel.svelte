<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { goto, invalidateAll } from "$app/navigation";
  import type { SnapshotSummary } from "$lib/roster";
  import { selectorOptions, snapshotSeqs } from "$lib/headBar";

  let {
    snapshots,
    baselineId,
    comparisonId,
  }: {
    snapshots: SnapshotSummary[];
    baselineId: number | null;
    comparisonId: number | null;
  } = $props();

  let seqs = $derived(snapshotSeqs(snapshots));
  let options = $derived(selectorOptions(snapshots, seqs));

  // Only one row can be mid-confirmation at a time — requesting delete on another row
  // just moves the confirmation, it doesn't stack.
  let pendingId = $state<number | null>(null);
  let deletingId = $state<number | null>(null);
  let error = $state("");

  function requestDelete(id: number) {
    error = "";
    pendingId = id;
  }

  function cancelDelete() {
    pendingId = null;
  }

  async function confirmDelete(id: number) {
    deletingId = id;
    error = "";
    try {
      await invoke("delete_snapshot", { id });
      pendingId = null;
      // ADR-0011: deleting the Snapshot currently used as Baseline or Comparison falls back
      // to latest-vs-previous — same rule as a new run (PD-24) — by dropping the query params
      // and letting +page.ts's load fall back to its own defaults. Otherwise the current
      // selection is untouched; invalidateAll just refreshes the snapshot list under it.
      if (id === baselineId || id === comparisonId) {
        await goto("/dashboard", { invalidateAll: true, keepFocus: true, noScroll: true });
      } else {
        await invalidateAll();
      }
    } catch (err) {
      error = String(err);
    } finally {
      deletingId = null;
    }
  }
</script>

<section class="snapshots-panel">
  <h3 class="panel-title">snapshots</h3>

  {#if options.length === 0}
    <p class="status">no runs recorded yet</p>
  {:else}
    <ul class="list">
      {#each options as option (option.id)}
        <li class="row">
          {#if pendingId === option.id}
            <span class="confirm-label">Delete {option.label}?</span>
            <span class="actions">
              <button
                type="button"
                class="danger"
                onclick={() => confirmDelete(option.id)}
                disabled={deletingId === option.id}
              >
                {deletingId === option.id ? "Deleting…" : "Confirm"}
              </button>
              <button
                type="button"
                class="secondary"
                onclick={cancelDelete}
                disabled={deletingId === option.id}
              >
                Cancel
              </button>
            </span>
          {:else}
            <span class="label">{option.label}</span>
            <button
              type="button"
              class="secondary"
              onclick={() => requestDelete(option.id)}
              aria-label={`Delete ${option.label}`}
            >
              Delete
            </button>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}

  {#if error}
    <p class="error">{error}</p>
  {/if}
</section>

<style>
  .snapshots-panel {
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

  .status {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--t-body-s);
    color: var(--ink-mute);
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: var(--s-5);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s-5);
  }

  .label {
    font-family: var(--font-mono);
    font-size: var(--t-body-s);
    color: var(--ink-2);
  }

  .confirm-label {
    flex: 1;
    min-width: 0;
    font-family: var(--font-sans);
    font-size: var(--t-body-s);
    color: var(--ink);
  }

  .actions {
    display: flex;
    gap: var(--s-5);
    flex: none;
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
  button.danger {
    border-color: var(--state-revoked);
    color: var(--state-revoked);
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
