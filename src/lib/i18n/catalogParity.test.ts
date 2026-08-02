import { describe, expect, it } from "vitest";
import en from "./en.json";
import ptBR from "./pt-BR.json";

function flattenKeys(node: unknown, prefix = ""): string[] {
  if (typeof node !== "object" || node === null) return [prefix];
  return Object.entries(node).flatMap(([key, value]) =>
    flattenKeys(value, prefix ? `${prefix}.${key}` : key),
  );
}

describe("catalog key parity", () => {
  it("has the same set of keys in pt-BR as the English source-of-truth catalog", () => {
    const enKeys = new Set(flattenKeys(en));
    const ptBRKeys = new Set(flattenKeys(ptBR));

    const missingFromPtBR = [...enKeys].filter((key) => !ptBRKeys.has(key)).sort();
    const staleInPtBR = [...ptBRKeys].filter((key) => !enKeys.has(key)).sort();

    expect(missingFromPtBR).toEqual([]);
    expect(staleInPtBR).toEqual([]);
  });
});
