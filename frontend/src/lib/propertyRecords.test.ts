import { describe, expect, it } from "vitest";
import {
  centsFrom,
  day,
  daysUntil,
  dollarsField,
  dueLabel,
  label,
  replacementKit,
} from "./propertyRecords";

describe("labels", () => {
  it("reads codes as words", () => {
    expect(label("builders_risk")).toBe("Builders risk");
    expect(label("k8")).toBe("K-8");
    expect(label("hvac")).toBe("HVAC");
  });
  it("reads a date as a calendar day", () => {
    expect(day("2026-03-05")).toMatch(/Mar 5, 2026/);
    expect(day(null)).toBe("");
  });
});

describe("due dates", () => {
  const today = new Date(2026, 2, 1);
  it("counts days", () => {
    expect(daysUntil("2026-03-04", today)).toBe(3);
    expect(daysUntil("2026-02-27", today)).toBe(-2);
  });
  it("says it plainly", () => {
    expect(dueLabel("2026-03-01", today)).toBe("Due today");
    expect(dueLabel("2026-03-02", today)).toBe("Due tomorrow");
    expect(dueLabel("2026-03-06", today)).toBe("Due in 5 days");
    expect(dueLabel("2026-02-28", today)).toBe("1 day late");
    expect(dueLabel("2026-02-20", today)).toBe("9 days late");
    expect(dueLabel(null, today)).toBe("");
  });
});

describe("replacementKit", () => {
  it("matches equipment to the job that replaces it", () => {
    expect(replacementKit("Kitchen dishwasher")).toBe("replace-dishwasher");
    expect(replacementKit("40-gal Water Heater")).toBe("replace-water-heater");
    expect(replacementKit("Gas furnace")).toBe("service-hvac");
    expect(replacementKit("AC — Unit 1A living room")).toBe("service-hvac");
    expect(replacementKit("Hall smoke alarm")).toBe(
      "replace-smoke-co-detectors"
    );
    expect(replacementKit("Garage door opener")).toBeNull();
  });
});

describe("money fields", () => {
  it("round-trips dollars", () => {
    expect(centsFrom("$1,840")).toBe(184000);
    expect(centsFrom("")).toBeNull();
    expect(centsFrom("-5")).toBeNull();
    expect(dollarsField(184000)).toBe("1840");
    expect(dollarsField(null)).toBe("");
  });
});
