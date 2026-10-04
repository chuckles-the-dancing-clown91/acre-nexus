import { describe, expect, it } from "vitest";
import { addDays, span } from "./campground";

describe("campground grid", () => {
  it("adds days across months", () => {
    expect(addDays("2026-01-30", 3)).toBe("2026-02-02");
  });
  it("places a stay on the day columns it covers", () => {
    expect(
      span(
        { check_in: "2026-07-03", check_out: "2026-07-06" },
        "2026-07-01",
        14
      )
    ).toEqual([2, 5]);
    // Starts before the window, ends inside it.
    expect(
      span(
        { check_in: "2026-06-28", check_out: "2026-07-02" },
        "2026-07-01",
        14
      )
    ).toEqual([0, 1]);
    // Entirely outside.
    expect(
      span(
        { check_in: "2026-08-01", check_out: "2026-08-03" },
        "2026-07-01",
        14
      )
    ).toBeNull();
  });
});
