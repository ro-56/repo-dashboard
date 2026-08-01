<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { save } from "@tauri-apps/plugin-dialog";
  import { writeTextFile } from "@tauri-apps/plugin-fs";
  import { SvelteSet } from "svelte/reactivity";
  import { goto } from "$app/navigation";
  import type { PageData } from "./$types";
  import type { RosterTreeResult } from "$lib/roster";
  import type { CredentialsSummary } from "$lib/credentials";
  import { exportFilename, rosterToCsv } from "$lib/rosterExport";
  import HeadBar from "$lib/components/HeadBar.svelte";
  import FilterBar from "$lib/components/FilterBar.svelte";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import ProjectSection from "$lib/components/ProjectSection.svelte";
  import RosterColumnLabels from "$lib/components/RosterColumnLabels.svelte";
  import SideDrawer from "$lib/components/SideDrawer.svelte";
  import CredentialsPanel from "$lib/components/CredentialsPanel.svelte";
  import RunPanel from "$lib/components/RunPanel.svelte";
  import SnapshotsPanel from "$lib/components/SnapshotsPanel.svelte";
  import AppearancePanel from "$lib/components/AppearancePanel.svelte";
  import ConfirmApplyDialog from "$lib/components/ConfirmApplyDialog.svelte";
  import { isRepoOpen, treeHasAnyChanges } from "$lib/repoCard";
  import { canApply, snapshotSeqs } from "$lib/headBar";
  import {
    allVisibleOpen,
    countNote,
    viewCounts,
    visibleRepoCount,
    visibleRepos,
    type ViewMode,
  } from "$lib/filterBar";
  import { emptyStateFor } from "$lib/emptyState";
  import {
    canRefresh,
    clearEdits,
    confirmRows,
    resultRows,
    settleApplied,
    stageEdit,
    successfulResults,
    toApplyPayload,
    undoEdit,
    type ApplyResult,
    type PendingEdits,
    type StagedEdit,
  } from "$lib/pendingEdits";

  let { data }: { data: PageData } = $props();

  let drawerOpen = $state(false);
  function toggleDrawer() {
    drawerOpen = !drawerOpen;
  }

  // Keys of repos the user has clicked away from their computed default-open state
  // (PD-16's default-expand rule) — not "which repos are open" directly, so that rule keeps
  // applying to every repo the user hasn't touched even as the underlying tree changes.
  let toggledRepos = new SvelteSet<string>();

  // All access / Changes only (PD-20). Kept independent of toggledRepos so switching tabs
  // restores whatever expansion state the user had, rather than resetting it.
  let viewMode = $state<ViewMode>("all");

  function repoKey(repoProject: string, repo: string): string {
    return `${repoProject}::${repo}`;
  }

  function toggleRepo(repoProject: string, repo: string) {
    const key = repoKey(repoProject, repo);
    if (toggledRepos.has(key)) {
      toggledRepos.delete(key);
    } else {
      toggledRepos.add(key);
    }
  }

  function isToggled(repoProject: string, repo: string): boolean {
    return toggledRepos.has(repoKey(repoProject, repo));
  }

  let anyChanges = $derived(treeHasAnyChanges(data.tree));
  let seqs = $derived(snapshotSeqs(data.snapshots));
  let same = $derived(data.baselineId === data.comparisonId);

  let counts = $derived(viewCounts(data.tree, viewMode));
  let allOpen = $derived(allVisibleOpen(data.tree, viewMode, anyChanges, isToggled));
  let expandLabel = $derived(allOpen ? "Collapse all" : "Expand all");
  let isEmpty = $derived(visibleRepoCount(data.tree, viewMode) === 0);
  let empty = $derived(
    isEmpty
      ? emptyStateFor(viewMode, data.pair!, data.comparison!, seqs.get(data.baselineId)!, seqs.get(data.comparisonId)!)
      : null,
  );

  function setViewMode(mode: ViewMode) {
    viewMode = mode;
  }

  // Flips every currently visible card whose open state doesn't already match `desiredOpen`,
  // leaving cards hidden by the current tab untouched (PD-20: "opens every visible card").
  function setBulkOpen(desiredOpen: boolean) {
    for (const project of data.tree) {
      for (const repo of visibleRepos(project, viewMode)) {
        const currentlyOpen = isRepoOpen(repo, anyChanges, isToggled(project.repoProject, repo.repo));
        if (currentlyOpen !== desiredOpen) {
          toggleRepo(project.repoProject, repo.repo);
        }
      }
    }
  }

  function handleBulkToggle() {
    setBulkOpen(!allOpen);
  }

  function resetView() {
    viewMode = "all";
    toggledRepos.clear();
  }

  function handleEmptyAction() {
    if (empty?.action === "show-all") {
      viewMode = "all";
    } else {
      resetView();
    }
  }

  function navigateToPair(baselineId: number, comparisonId: number) {
    const params = new URLSearchParams({ baseline: String(baselineId), comparison: String(comparisonId) });
    goto(`?${params.toString()}`, { keepFocus: true, noScroll: true });
  }

  function selectBaseline(id: number) {
    navigateToPair(id, data.comparisonId);
  }
  function selectComparison(id: number) {
    navigateToPair(data.baselineId, id);
  }
  function swapPair() {
    navigateToPair(data.comparisonId, data.baselineId);
  }

  // Permission editing (PD-59/PD-60). Enabled only while the Comparison is the latest
  // Snapshot (ADR-0022) — editing against a historical view would mutate the present based on
  // a look at the past.
  let editingEnabled = $derived(data.snapshots.length > 0 && data.comparisonId === data.snapshots[0].id);
  let pending = $state<PendingEdits>(new Map());

  // Clears whenever the Comparison itself changes — covers both "user picked an older run"
  // and "a new Run completed and became the new latest" (ADR-0022), since either way the
  // pending edits were staged against a Comparison that's no longer the one on screen.
  $effect(() => {
    data.comparisonId;
    pending = clearEdits();
  });

  function handleStage(edit: StagedEdit) {
    pending = stageEdit(pending, edit);
  }
  function handleUndo(key: string) {
    pending = undoEdit(pending, key);
  }

  let pendingCount = $derived(pending.size);
  let applyIsEnabled = $derived(canApply(pendingCount, editingEnabled));

  type DialogPhase = "review" | "applying" | "results";
  let dialogOpen = $state(false);
  let dialogPhase = $state<DialogPhase>("review");
  // Snapshotted at the moment Apply is clicked, so the confirm list (and later, the results
  // view matched against it) stays stable even if `pending` changes underneath while the
  // dialog is open.
  let dialogSnapshot = $state<PendingEdits>(new Map());
  let dialogResults = $state<ApplyResult[] | null>(null);
  let dialogError = $state<string | null>(null);
  // The Snapshot the batch's edits were staged against — `editingEnabled` only allows staging
  // while `data.comparisonId` is the latest Snapshot, so that's exactly the Snapshot Refresh
  // must copy from.
  let dialogSourceSnapshotId = $state<number | null>(null);
  let refreshing = $state(false);

  let dialogRows = $derived(confirmRows(dialogSnapshot));
  let dialogResultRows = $derived(dialogResults ? resultRows(dialogSnapshot, dialogResults) : []);
  let dialogCanRefresh = $derived(dialogResults !== null && canRefresh(dialogResults));

  function openApplyDialog() {
    if (!applyIsEnabled) return;
    dialogSnapshot = new Map(pending);
    dialogSourceSnapshotId = data.comparisonId;
    dialogPhase = "review";
    dialogResults = null;
    dialogError = null;
    dialogOpen = true;
  }

  function cancelApplyDialog() {
    dialogOpen = false;
  }

  async function confirmApply() {
    dialogPhase = "applying";
    dialogError = null;
    try {
      const results = await invoke<ApplyResult[]>("apply_pending_edits", {
        edits: toApplyPayload(dialogSnapshot),
      });
      dialogResults = results;
      pending = settleApplied(pending, results);
      dialogPhase = "results";
    } catch (err) {
      dialogError = String(err);
      dialogPhase = "review";
    }
  }

  function closeApplyDialog() {
    dialogOpen = false;
  }

  // Refresh (PD-70): a narrow, Repo-scope-only copy-plus-fetch, offered right in the results
  // view. On success it clears the remaining Pending edits exactly as a Run does, refreshes the
  // snapshot list, and auto-selects the new Snapshot as the Comparison (baseline untouched).
  async function refreshAffected() {
    if (dialogResults === null || dialogSourceSnapshotId === null) return;
    refreshing = true;
    dialogError = null;
    try {
      const newSnapshotId = await invoke<number>("refresh_snapshot", {
        sourceSnapshotId: dialogSourceSnapshotId,
        results: successfulResults(dialogResults),
      });
      dialogOpen = false;
      pending = clearEdits();
      const params = new URLSearchParams({
        baseline: String(data.baselineId),
        comparison: String(newSnapshotId),
      });
      await goto(`?${params.toString()}`, { invalidateAll: true, keepFocus: true, noScroll: true });
    } catch (err) {
      dialogError = String(err);
    } finally {
      refreshing = false;
    }
  }

  // Export (PD-72/PD-74): always the Comparison Snapshot alone, diff-free and unfiltered —
  // fetched fresh via get_roster_tree(comparisonId, comparisonId) rather than reusing data.tree,
  // since the dashboard's own tree is diffed against whatever Baseline is currently selected.
  async function handleExport() {
    const comparisonSnapshot = data.snapshots.find((s) => s.id === data.comparisonId);
    if (!comparisonSnapshot) return;

    const [result, credentials] = await Promise.all([
      invoke<RosterTreeResult>("get_roster_tree", {
        snapshotAId: data.comparisonId,
        snapshotBId: data.comparisonId,
      }),
      invoke<CredentialsSummary>("get_credentials"),
    ]);

    const filename = exportFilename(credentials.workspace ?? "workspace", comparisonSnapshot.runAt);
    const path = await save({ defaultPath: filename, filters: [{ name: "CSV", extensions: ["csv"] }] });
    if (!path) return;

    await writeTextFile(path, rosterToCsv(result.tree, comparisonSnapshot.runAt));
  }
</script>

<main class="dashboard">
  <HeadBar
    snapshots={data.snapshots}
    baselineId={data.baselineId}
    comparisonId={data.comparisonId}
    comparison={data.comparison}
    pair={data.pair}
    {same}
    onSelectBaseline={selectBaseline}
    onSelectComparison={selectComparison}
    onSwap={swapPair}
    {drawerOpen}
    onToggleDrawer={toggleDrawer}
    {pendingCount}
    canApply={applyIsEnabled}
    onApplyClick={openApplyDialog}
  />
  {#if data.snapshots.length === 0}
    <div class="canvas">
      <p class="empty-note">No runs recorded yet — open the settings drawer to connect Bitbucket credentials.</p>
    </div>
  {:else}
    <FilterBar
      {viewMode}
      countNoteText={countNote(counts)}
      {expandLabel}
      bulkDisabled={isEmpty}
      onSetViewMode={setViewMode}
      onBulkToggle={handleBulkToggle}
      onReset={resetView}
      onExportClick={handleExport}
    />

    {#if !isEmpty}
      <RosterColumnLabels />
    {/if}
    <div class="canvas">
      {#each data.tree as project (project.repoProject)}
        <ProjectSection
          {project}
          comparisonId={data.comparisonId}
          {anyChanges}
          {viewMode}
          {isToggled}
          onToggle={toggleRepo}
          {pending}
          {editingEnabled}
          onStage={handleStage}
          onUndo={handleUndo}
        />
      {/each}
      {#if empty}
        <EmptyState state={empty} onAction={handleEmptyAction} />
      {/if}
    </div>
  {/if}
</main>

<SideDrawer open={drawerOpen} onClose={toggleDrawer}>
  <CredentialsPanel />
  <RunPanel />
  <SnapshotsPanel snapshots={data.snapshots} baselineId={data.baselineId} comparisonId={data.comparisonId} />
  <AppearancePanel />
</SideDrawer>

<ConfirmApplyDialog
  open={dialogOpen}
  phase={dialogPhase}
  rows={dialogRows}
  resultRows={dialogResultRows}
  errorMessage={dialogError}
  canRefresh={dialogCanRefresh}
  {refreshing}
  onConfirm={confirmApply}
  onCancel={cancelApplyDialog}
  onClose={closeApplyDialog}
  onRefresh={refreshAffected}
/>

<style>
  .dashboard {
    display: flex;
    flex-direction: column;
    height: 100dvh;
    overflow: hidden;
    font-family: var(--font-sans);
    color: var(--ink);
  }

  .empty-note {
    margin: var(--s-7);
    font-family: var(--font-mono);
    font-size: var(--t-body);
    color: var(--ink-3);
  }

  .canvas {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    background: var(--canvas);
    padding: var(--s-7) var(--s-7) var(--s-8);
  }
</style>
