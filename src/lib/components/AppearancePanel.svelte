<script lang="ts">
  import { _ } from "svelte-i18n";
  import { getLanguage, setLanguage, type Language } from "$lib/language";
  import { getTheme, setTheme, type Theme } from "$lib/theme";

  let current = $state<Theme>(getTheme());
  let currentLanguage = $state<Language>(getLanguage());

  function choose(theme: Theme) {
    setTheme(theme);
    current = theme;
  }

  function chooseLanguage(language: Language) {
    setLanguage(language);
    currentLanguage = language;
  }
</script>

<section class="appearance-panel">
  <h3 class="panel-title">{$_("appearance.title")}</h3>
  <div class="segmented">
    <button type="button" class="tab" class:active={current === "light"} onclick={() => choose("light")}>
      Light
    </button>
    <button type="button" class="tab" class:active={current === "dark"} onclick={() => choose("dark")}>
      Dark
    </button>
  </div>
  <div class="segmented">
    <button type="button" class="tab" class:active={currentLanguage === "en"} onclick={() => chooseLanguage("en")}>
      English
    </button>
    <button
      type="button"
      class="tab"
      class:active={currentLanguage === "pt-BR"}
      onclick={() => chooseLanguage("pt-BR")}
    >
      Português (BR)
    </button>
  </div>
</section>

<style>
  .appearance-panel {
    display: flex;
    flex-direction: column;
    gap: var(--s-5);
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

  .segmented {
    display: flex;
    align-self: flex-start;
    padding: 2px;
    border-radius: var(--r-card);
    background: var(--track);
  }
  .tab {
    display: inline-flex;
    align-items: center;
    height: 28px;
    padding: 0 var(--s-7);
    border: none;
    border-radius: var(--r-row);
    background: none;
    font-family: var(--font-sans);
    font-weight: 500;
    font-size: var(--t-body-s);
    color: var(--ink-4);
    cursor: pointer;
  }
  .tab.active {
    background: var(--track-active);
    box-shadow: var(--shadow-segment);
    font-weight: 600;
    color: var(--ink);
  }
</style>
