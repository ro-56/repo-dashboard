<script lang="ts">
  import "$lib/styles/tokens.css";
  import { isLoading } from "svelte-i18n";
  import { initLanguage } from "$lib/language";
  import { initTheme } from "$lib/theme";

  initTheme();
  initLanguage();

  let { children } = $props();
</script>

{#if !$isLoading}
  {@render children()}
{/if}

<style>
  :global(html),
  :global(body) {
    height: 100%;
    margin: 0;
  }
  :global(body) {
    background: var(--canvas);
  }
</style>
