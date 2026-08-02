import { describe, expect, it } from "vitest";
import { englishTranslate } from "./englishTranslate";

describe("englishTranslate", () => {
  it("resolves a nested dotted key against the English catalog", () => {
    expect(englishTranslate("permission.admin")).toBe("Admin");
  });

  it("interpolates values, including ICU plural forms", () => {
    expect(englishTranslate("repoCard.tag.goneLabel", { values: { id: 5 } })).toBe("absent from run 5");
    expect(englishTranslate("roster.menu.cascadeNote", { values: { count: 1 } })).toBe(
      "removing this also drops access for 1 member",
    );
    expect(englishTranslate("roster.menu.cascadeNote", { values: { count: 3 } })).toBe(
      "removing this also drops access for 3 members",
    );
  });

  it("throws on a key that isn't in the English catalog", () => {
    expect(() => englishTranslate("not.a.real.key")).toThrow(/Missing English catalog key/);
  });
});
