<script lang="ts">
  import type { PrincipalEntry, RepoNode } from "$lib/roster";
  import { sortedPrincipals } from "$lib/rosterRow";
  import {
    absentSide,
    countBadges,
    defaultOpen,
    distributionBarBackground,
    grantsText,
    repoBreakdown,
    repoTag,
  } from "$lib/repoCard";
  import CountBadges from "./CountBadges.svelte";
  import RosterRow from "./RosterRow.svelte";

  let {
    repo,
    comparisonId,
    anyChanges,
    toggled,
    onToggle,
  }: {
    repo: RepoNode;
    comparisonId: number;
    anyChanges: boolean;
    toggled: boolean;
    onToggle: () => void;
  } = $props();

  // `toggled` records whether the user has clicked this card away from its computed default;
  // XOR-ing against the default keeps that default live as the underlying tree data changes.
  let open = $derived(defaultOpen(repo, anyChanges) !== toggled);
  let counts = $derived(countBadges(repoBreakdown(repo)));
  let tag = $derived(repoTag(repo, comparisonId));
  let gone = $derived(absentSide(repo) === "B");

  // A principal can appear more than once per repo (e.g. Direct plus Member-of-group-X),
  // so the key needs the access type — and, for Member, the group_id — to stay unique.
  function principalEntryKey(entry: PrincipalEntry): string {
    const groupId = entry.accessType.type === "Member" ? entry.accessType.group_id : "";
    return `${entry.principal.id}::${entry.accessType.type}::${groupId}`;
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
      <span class="bar" style:background={distributionBarBackground(repo)}></span>
    </span>
    <span class="grants">{grantsText(repo)}</span>
    <CountBadges badges={counts} />
    {#if tag}
      <span class="tag tag-{tag.kind}">{tag.label}</span>
    {/if}
  </button>

  {#if open}
    <div class="roster">
      {#each sortedPrincipals(repo.principals) as entry (principalEntryKey(entry))}
        <RosterRow {entry} />
      {/each}
    </div>
  {/if}
</article>

<style>
  .repo-card {
    background: var(--surface);
    border: 1px solid var(--rule);
    border-radius: var(--radius);
    overflow: hidden;
  }

  .repo-head {
    box-sizing: border-box;
    display: grid;
    grid-template-columns: 12px 232px 62px 148px 1fr auto;
    gap: 10px;
    align-items: center;
    width: 100%;
    height: var(--card-h);
    padding: 0 var(--s-6);
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
  .bar {
    display: inline-block;
    width: 58px;
    height: 6px;
    border-radius: 1px;
  }

  .grants {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: var(--t-meta);
    color: var(--ink-3);
  }

  .tag {
    display: inline-flex;
    align-items: center;
    height: 14px;
    padding: 0 var(--s-3);
    border-radius: var(--radius);
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

  .roster {
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--rule-row);
  }
</style>
