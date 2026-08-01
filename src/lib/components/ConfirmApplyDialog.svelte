<script lang="ts">
  import type { ConfirmRow, ResultRow } from "$lib/pendingEdits";
  import { cascadeNote } from "$lib/rosterRow";

  let {
    open,
    phase,
    rows,
    resultRows,
    errorMessage,
    onConfirm,
    onCancel,
    onClose,
  }: {
    open: boolean;
    phase: "review" | "applying" | "results";
    rows: ConfirmRow[];
    resultRows: ResultRow[];
    errorMessage: string | null;
    onConfirm: () => void;
    onCancel: () => void;
    onClose: () => void;
  } = $props();
</script>

{#if open}
  <div class="overlay">
    <div class="dialog" role="dialog" aria-modal="true" aria-label="Apply pending edits">
      {#if phase === "results"}
        <h2 class="title">Apply results</h2>
        <ul class="rows">
          {#each resultRows as row (row.key)}
            <li class="row">
              <span class="path">{row.repoProject}/{row.repo}</span>
              <span class="principal">{row.principalLabel}</span>
              <span class="transition">{row.fromLevel} → {row.toLevel}</span>
              {#if row.ok}
                <span class="status status-ok">✓ applied</span>
              {:else}
                <span class="status status-fail">✕ {row.errorMessage}</span>
              {/if}
            </li>
          {/each}
        </ul>
        <p class="rerun-note">Re-run to see this reflected — a Snapshot never updates itself.</p>
        <div class="actions">
          <button type="button" class="btn btn-primary" onclick={onClose}>Close</button>
        </div>
      {:else}
        <h2 class="title">Apply {rows.length} {rows.length === 1 ? "change" : "changes"}</h2>
        <ul class="rows">
          {#each rows as row (row.key)}
            <li class="row-item">
              <div class="row">
                <span class="path">{row.repoProject}/{row.repo}</span>
                <span class="principal">{row.principalLabel}</span>
                <span class="transition">{row.fromLevel} → {row.toLevel}</span>
              </div>
              {#if row.cascadeCount != null}
                <div class="cascade-note">{cascadeNote(row.cascadeCount)}</div>
              {/if}
            </li>
          {/each}
        </ul>
        {#if errorMessage}
          <p class="error">{errorMessage}</p>
        {/if}
        <div class="actions">
          <button type="button" class="btn btn-ghost" disabled={phase === "applying"} onclick={onCancel}>
            Cancel
          </button>
          <button type="button" class="btn btn-primary" disabled={phase === "applying"} onclick={onConfirm}>
            {phase === "applying" ? "Applying…" : "Confirm"}
          </button>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--scrim);
  }

  .dialog {
    display: flex;
    flex-direction: column;
    gap: var(--s-6);
    width: 480px;
    max-height: 70vh;
    padding: var(--s-8);
    border-radius: var(--r-card);
    background: var(--surface);
    box-shadow: var(--shadow-drawer);
  }

  .title {
    margin: 0;
    font-family: var(--font-sans);
    font-weight: 600;
    font-size: var(--t-stat);
    color: var(--ink);
  }

  .rows {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: var(--s-5);
    overflow-y: auto;
  }

  .row-item {
    display: flex;
    flex-direction: column;
    gap: var(--s-2);
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--s-5);
    font-family: var(--font-mono);
    font-size: var(--t-body-s);
    color: var(--ink);
  }
  .cascade-note {
    font-family: var(--font-sans);
    font-size: var(--t-tag);
    color: var(--state-revoked);
  }
  .path {
    flex: none;
    color: var(--ink-3);
    white-space: nowrap;
  }
  .principal {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }
  .transition {
    flex: none;
    margin-left: auto;
    color: var(--ink-2);
    white-space: nowrap;
  }
  .status {
    flex: none;
    font-weight: 600;
    white-space: nowrap;
  }
  .status-ok {
    color: var(--state-added);
  }
  .status-fail {
    color: var(--state-revoked);
  }

  .rerun-note {
    margin: 0;
    font-family: var(--font-sans);
    font-size: var(--t-body-s);
    color: var(--ink-3);
  }
  .error {
    margin: 0;
    font-family: var(--font-sans);
    font-size: var(--t-body-s);
    color: var(--state-revoked);
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--s-4);
  }
  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: var(--h-control);
    padding: 0 var(--s-6);
    border-radius: var(--r-chip);
    font-family: var(--font-sans);
    font-weight: 500;
    font-size: var(--t-body-s);
    cursor: pointer;
  }
  .btn:disabled {
    cursor: default;
    opacity: 0.6;
  }
  .btn-ghost {
    border: 1px solid var(--control-edge);
    background: var(--surface);
    color: var(--ink-3);
  }
  .btn-primary {
    border: 1px solid var(--ink);
    background: var(--ink);
    color: var(--surface);
  }
</style>
