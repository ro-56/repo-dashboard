<!-- Column-label strip for the roster grid (ADR-0006). Rendered once above the canvas, not
     per card — its grid tracks mirror RosterRow.svelte's exactly so labels stay aligned. -->
<div class="column-labels">
  <span>±</span>
  <span>lvl</span>
  <span>access</span>
  <span>source</span>
  <span>user</span>
  <span>since baseline</span>
  <span>notes</span>
</div>

<style>
  .column-labels {
    box-sizing: border-box;
    display: grid;
    grid-template-columns: 12px 26px 46px 70px 148px 132px 1fr;
    gap: var(--s-5);
    align-items: center;
    padding: var(--s-3) var(--s-7) var(--s-2) 25px;
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
