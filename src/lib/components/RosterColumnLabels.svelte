<!-- Column-label strip for the roster grid (ADR-0006). Rendered once above the canvas, not
     per card — its grid tracks mirror RosterRow.svelte's exactly so labels stay aligned. -->
<div class="column-labels">
  <span title="Diff outcome">±</span>
  <span title="Permission level">lvl</span>
  <span title="Permission level">access</span>
  <span title="How this principal holds the grant">source</span>
  <span title="The user or group">user</span>
  <span title="What changed vs. the baseline">since baseline</span>
  <span>notes</span>
</div>

<style>
  .column-labels {
    box-sizing: border-box;
    display: grid;
    grid-template-columns: var(--roster-cols);
    gap: var(--s-6);
    align-items: center;
    padding: var(--s-4) var(--s-8) var(--s-4) 30px;
    background: var(--canvas-head);
    border-bottom: 1px solid var(--rule);
    font-family: var(--font-sans);
    font-size: var(--t-micro);
    font-weight: 500;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--ink-mute);
  }
</style>
