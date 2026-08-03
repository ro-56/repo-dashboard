// Whole-dashboard display language (ADR-0028). Wraps svelte-i18n's locale store so
// components consume translations through the `$_` store alone, mirroring theme.ts's shape.

import { init, locale as i18nLocale, register } from "svelte-i18n";

export type Language = "en" | "pt-BR";

const STORAGE_KEY = "language";
const SUPPORTED: readonly Language[] = ["en", "pt-BR"];
const DEFAULT_LANGUAGE: Language = "en";

register("en", () => import("./i18n/en.json"));
register("pt-BR", () => import("./i18n/pt-BR.json"));

let current: Language = DEFAULT_LANGUAGE;
let initialized = false;

function applyLanguage(language: Language): void {
  if (!initialized) {
    init({ fallbackLocale: DEFAULT_LANGUAGE, initialLocale: language });
    initialized = true;
    return;
  }
  i18nLocale.set(language);
}

export function getLanguage(): Language {
  return current;
}

export function setLanguage(language: Language): void {
  current = language;
  localStorage.setItem(STORAGE_KEY, language);
  applyLanguage(language);
}

function matchSupported(tag: string | null | undefined): Language | null {
  if (!tag) return null;
  const lower = tag.toLowerCase();
  return SUPPORTED.find((lang) => lower.startsWith(lang.toLowerCase()) || lower.startsWith(lang.split("-")[0].toLowerCase())) ?? null;
}

/** One-time OS/webview locale read, only ever consulted when nothing is stored. Prefers the
 * webview's `navigator.language`; falls back to `@tauri-apps/plugin-os`'s `locale()` (the
 * OS-level locale, needed when the webview doesn't expose one) if that's inconclusive. */
async function detectOsLanguage(): Promise<Language> {
  const navLanguage = typeof navigator !== "undefined" ? navigator.language : undefined;
  const fromNav = matchSupported(navLanguage);
  if (fromNav) return fromNav;

  try {
    const { locale } = await import("@tauri-apps/plugin-os");
    const osLocale = await locale();
    return matchSupported(osLocale) ?? DEFAULT_LANGUAGE;
  } catch {
    return DEFAULT_LANGUAGE;
  }
}

/** Reads the stored choice first — an explicit choice always wins. Only when nothing is
 * stored does this fall back to a one-time OS/webview locale read, never a live listener, so a
 * later OS-level change can never override an explicit stored choice.
 *
 * The catalog is always applied synchronously before this returns — with a stored choice
 * that's the final language, otherwise it's `DEFAULT_LANGUAGE` as an immediate placeholder so
 * `$_` never renders blank/untranslated, and the OS-detected language (if different) lands a
 * tick later once `detectOsLanguage` resolves. */
export function initLanguage(): Promise<Language> {
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored === "en" || stored === "pt-BR") {
    current = stored;
    applyLanguage(stored);
    return Promise.resolve(stored);
  }

  current = DEFAULT_LANGUAGE;
  applyLanguage(DEFAULT_LANGUAGE);
  return detectOsLanguage().then((language) => {
    current = language;
    applyLanguage(language);
    return language;
  });
}
