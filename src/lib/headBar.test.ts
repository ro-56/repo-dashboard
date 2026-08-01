import { describe, expect, it } from "vitest";
import { canApply } from "./headBar";

describe("canApply", () => {
  it("is true when there's staged work and editing is enabled", () => {
    expect(canApply(1, true)).toBe(true);
  });

  it("is false when there's nothing staged, even if editing is enabled", () => {
    expect(canApply(0, true)).toBe(false);
  });

  it("is false when editing is disabled, even with staged work", () => {
    expect(canApply(1, false)).toBe(false);
  });

  it("is false when there's nothing staged and editing is disabled", () => {
    expect(canApply(0, false)).toBe(false);
  });
});
