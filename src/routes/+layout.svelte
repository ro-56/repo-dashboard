<script lang="ts">
  import "$lib/styles/tokens.css";
  import { initTheme } from "$lib/theme";

  initTheme();

  let { children } = $props();
</script>

{@render children()}

<style>
  :global(html),
  :global(body) {
    height: 100%;
    margin: 0;
  }
</style>
