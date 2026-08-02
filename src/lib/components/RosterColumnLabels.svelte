<!-- Column-label strip for the roster grid (ADR-0006). Rendered once above the canvas, not
     per card — its grid tracks mirror RosterRow.svelte's exactly so labels stay aligned. -->
<script lang="ts">
  import { _ } from "svelte-i18n";
</script>

<div class="column-labels">
  <span title={$_("roster.columns.diffTooltip")}>{$_("roster.columns.diff")}</span>
  <span title={$_("roster.columns.levelTooltip")}>{$_("roster.columns.level")}</span>
  <span title={$_("roster.columns.levelTooltip")}>{$_("roster.columns.access")}</span>
  <span title={$_("roster.columns.sourceTooltip")}>{$_("roster.columns.source")}</span>
  <span title={$_("roster.columns.userTooltip")}>{$_("roster.columns.user")}</span>
  <span title={$_("roster.columns.sinceBaselineTooltip")}>{$_("roster.columns.sinceBaseline")}</span>
  <span>{$_("roster.columns.notes")}</span>
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
