<script lang="ts">
  import type { PrincipalEntry, RepoNode } from "$lib/roster";
  import { hasDiff, sortedPrincipals } from "$lib/rosterRow";
  import {
    absentSide,
    countBadges,
    distributionSplit,
    grantsText,
    isRepoOpen,
    repoBreakdown,
    repoTags,
  } from "$lib/repoCard";
  import type { ViewMode } from "$lib/filterBar";
  import type { PendingEdits, StagedEdit } from "$lib/pendingEdits";
  import CountBadges from "./CountBadges.svelte";
  import RosterRow from "./RosterRow.svelte";

  let {
    repo,
    comparisonId,
    anyChanges,
    viewMode,
    toggled,
    onToggle,
    pending,
    editingEnabled,
    onStage,
    onUndo,
  }: {
    repo: RepoNode;
    comparisonId: number;
    anyChanges: boolean;
    viewMode: ViewMode;
    toggled: boolean;
    onToggle: () => void;
    pending: PendingEdits;
    editingEnabled: boolean;
    onStage: (edit: StagedEdit) => void;
    onUndo: (key: string) => void;
  } = $props();

  // `toggled` records whether the user has clicked this card away from its computed default;
  // XOR-ing against the default keeps that default live as the underlying tree data changes.
  let open = $derived(isRepoOpen(repo, anyChanges, toggled));
  // Header counts always describe the whole repo (ADR-0008) — only the rendered rows narrow.
  let counts = $derived(countBadges(repoBreakdown(repo)));
  let split = $derived(distributionSplit(repo));
  let tags = $derived(repoTags(repo, comparisonId));
  let gone = $derived(absentSide(repo) === "B");
  let sorted = $derived(sortedPrincipals(repo.principals));
  let roster = $derived(viewMode === "changes" ? sorted.filter(hasDiff) : sorted);

  // A principal can appear more than once per repo (e.g. Direct plus Member-of-group-X, or
  // the same access type at both Repo and Project scope per PD-30), so the key needs the
  // access type — and, for Member, the group_id — and the scope to stay unique.
  function principalEntryKey(entry: PrincipalEntry): string {
    const groupId = entry.accessType.type === "Member" ? entry.accessType.group_id : "";
    return `${entry.principal.id}::${entry.accessType.type}::${groupId}::${entry.scope}`;
  }
</script>

<article class="repo-card">
  <button
    type="button"
    class="repo-head"
    class:open
    aria-expanded={open}
    onclick={onToggle}
  >
    <span class="caret">{open ? "▼" : "▶"}</span>
    <span class="path" class:gone>
      {repo.repoProject}/<span class="name" class:gone>{repo.repo}</span>
    </span>
    <span class="bar-cell">
      {#if split}
        <span class="dist">
          <span class="dist__admin" style:width={`${split.admin}%`}></span>
          <span class="dist__write" style:width={`${split.write}%`}></span>
          <span class="dist__read" style:width={`${split.read}%`}></span>
        </span>
      {:else}
        <span class="bar-flat"></span>
      {/if}
    </span>
    <span class="grants">{grantsText(repo)}</span>
    <CountBadges badges={counts} />
    <span class="tags">
      {#each tags as tag (tag.kind)}
        <span class="tag tag-{tag.kind}">{tag.label}</span>
      {/each}
    </span>
  </button>

  {#if open}
    <div class="roster">
      {#each roster as entry (principalEntryKey(entry))}
        <RosterRow
          {entry}
          repoProject={repo.repoProject}
          repo={repo.repo}
          {pending}
          {editingEnabled}
          {onStage}
          {onUndo}
        />
      {/each}
    </div>
  {/if}
</article>

<style>
  .repo-card {
    background: var(--surface);
    border: 1px solid var(--rule);
    border-radius: var(--r-card);
    overflow: hidden;
  }

  .repo-head {
    box-sizing: border-box;
    display: grid;
    grid-template-columns: 11px 250px 74px 148px 1fr auto;
    gap: var(--s-7);
    align-items: center;
    width: 100%;
    height: var(--card-h);
    padding: 0 var(--s-7);
    border: none;
    background: var(--surface);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .repo-head.open {
    background: var(--surface-raised);
    border-bottom: 1px solid var(--rule-row);
  }

  .caret {
    font-family: var(--font-mono);
    font-size: 8px;
    color: var(--ink-mute);
  }

  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: var(--t-body);
    color: var(--ink-mute);
  }
  .path.gone {
    text-decoration: line-through;
    text-decoration-color: var(--strike);
  }
  .name {
    font-weight: 600;
    color: var(--ink);
  }
  .name.gone {
    color: var(--state-revoked);
  }

  .bar-cell {
    display: flex;
    align-items: center;
  }
  .dist {
    display: flex;
    gap: 2px;
    width: 74px;
    height: 8px;
  }
  .dist span {
    border-radius: 2px;
  }
  .dist__admin {
    background: var(--bar-admin);
  }
  .dist__write {
    background: var(--bar-write);
  }
  .dist__read {
    background: var(--bar-read);
  }
  .bar-flat {
    display: inline-block;
    width: 74px;
    height: 8px;
    border-radius: 2px;
    background: var(--rule-row);
  }

  .grants {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: var(--t-meta);
    color: var(--ink-3);
  }

  .tags {
    display: flex;
    gap: var(--s-3);
    align-items: center;
  }
  .tag {
    display: inline-flex;
    align-items: center;
    height: var(--h-tag);
    padding: 0 var(--s-3);
    border-radius: var(--r-chip);
    font-family: var(--font-sans);
    font-size: var(--t-tag);
    font-weight: 500;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    white-space: nowrap;
    border: 1px solid;
  }
  .tag-gone {
    background: var(--tag-gone-bg);
    border-color: var(--tag-gone-border);
    color: var(--tag-gone-ink);
  }
  .tag-new {
    background: var(--tag-new-bg);
    border-color: var(--tag-new-border);
    color: var(--tag-new-ink);
  }
  .tag-failed {
    background: var(--tag-neutral-bg);
    border-color: var(--tag-neutral-border);
    color: var(--tag-neutral-ink);
  }
  /* Neutral palette, same as .tag-failed (ADR-0005: a fetch failure is not a diff outcome, so
     it may not claim a hue of its own) — a dashed border is the shape distinction from the
     repo-level tag, not a new colour, since both can render side by side. */
  .tag-project-failed {
    background: var(--tag-neutral-bg);
    border-style: dashed;
    border-color: var(--tag-neutral-border);
    color: var(--tag-neutral-ink);
  }

  .roster {
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--rule-row);
  }

  /* Reflow order (ADR-0009): grants text sheds first, then the distribution bar. Thresholds are
     unchanged from v1 (1300/1200) — the v2 mockup's own reflow block keeps these same breakpoints
     despite its wider path/bar tracks, so there's no evidence the new widths require a bump. */
  @media (max-width: 1300px) {
    .repo-head {
      grid-template-columns: 11px 250px 74px 1fr auto;
    }
    .grants {
      display: none;
    }
  }

  @media (max-width: 1200px) {
    .repo-head {
      grid-template-columns: 11px 250px 1fr auto;
    }
    .bar-cell {
      display: none;
    }
  }
</style>
