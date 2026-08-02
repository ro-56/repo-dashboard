import { describe, expect, it } from "vitest";
import { countNote, type ViewCounts } from "./filterBar";
import { t } from "./i18n/testHelpers";

describe("countNote", () => {
  it("renders the live/changed/rows counts", () => {
    const counts: ViewCounts = { live: 4, changed: 2, rows: 6 };
    expect(countNote(t, counts)).toBe("4 live · 2 changed · 6 rows");
  });
});
