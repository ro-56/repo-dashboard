// A `Translate` that always resolves against the English catalog, independent of the app's
// active Display language — for the handful of call sites that must stay English regardless
// (CSV Export, ADR-0030; the not-yet-localized ConfirmApplyDialog).
import { getMessageFormatter } from "svelte-i18n";
import en from "./en.json";
import type { Translate } from "./translate";

function lookup(id: string): string {
  let node: unknown = en;
  for (const part of id.split(".")) {
    node = (node as Record<string, unknown> | undefined)?.[part];
  }
  if (typeof node !== "string") throw new Error(`Missing English catalog key: ${id}`);
  return node;
}

export const englishTranslate: Translate = (id, options) =>
  getMessageFormatter(lookup(id), "en").format(options?.values) as string;
