// Whole-dashboard light/dark theme (ADR-0016). Applied via a `data-theme` attribute
// on the root <html> element so every component re-skins through tokens.css custom properties
// alone — no per-component JS branching (PRD, ADR-0005).

export type Theme = "light" | "dark";

const STORAGE_KEY = "theme";

let current: Theme = "light";

function applyTheme(theme: Theme): void {
  document.documentElement.setAttribute("data-theme", theme);
}

export function getTheme(): Theme {
  return current;
}

export function setTheme(theme: Theme): void {
  current = theme;
  localStorage.setItem(STORAGE_KEY, theme);
  applyTheme(theme);
}

/** Reads the stored choice first — an explicit choice always wins. Only when nothing is
 * stored does this fall back to a one-time prefers-color-scheme read, never a live matchMedia
 * listener, so a later OS-level change can never override an explicit stored choice. */
export function initTheme(): Theme {
  const stored = localStorage.getItem(STORAGE_KEY);
  const theme: Theme =
    stored === "light" || stored === "dark" ? stored : matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  current = theme;
  applyTheme(theme);
  return theme;
}
