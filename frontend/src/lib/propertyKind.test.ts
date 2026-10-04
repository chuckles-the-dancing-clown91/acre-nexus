import { describe, expect, it } from "vitest";
import { kindOf, normalizeKind, tabsFor } from "./propertyKind";

describe("property kinds", () => {
  it("collapses spellings", () => {
    expect(normalizeKind("Multi-Family")?.key).toBe("multi_family");
    expect(normalizeKind("apartment")?.key).toBe("multi_family");
    expect(normalizeKind("RV park")?.key).toBe("rv_park");
    expect(normalizeKind("spaceship")).toBeNull();
  });
  it("guesses a blank type from the unit count", () => {
    expect(kindOf({ property_type: "", units: 12 }).key).toBe("multi_family");
    expect(kindOf({ property_type: null, units: 1 }).key).toBe("single_family");
    expect(kindOf({ property_type: "condo", units: 40 }).key).toBe("condo");
  });
  it("gives each kind its own tabs", () => {
    expect(tabsFor(kindOf({ property_type: "multi_family" }))).toContain(
      "units"
    );
    expect(tabsFor(kindOf({ property_type: "single_family" }))).not.toContain(
      "units"
    );
    expect(tabsFor(kindOf({ property_type: "land" }))).not.toContain("meters");
  });
});
