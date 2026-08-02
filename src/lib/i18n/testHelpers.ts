// Shared test-only translate function backed by the real English catalog — so tests exercise
// the actual catalog strings (interpolation, ICU plurals) rather than duplicating them by hand.
import { _, addMessages, init } from "svelte-i18n";
import { get } from "svelte/store";
import en from "./en.json";
import type { Translate } from "./translate";

addMessages("en", en);
init({ fallbackLocale: "en", initialLocale: "en" });

export const t: Translate = get(_);
