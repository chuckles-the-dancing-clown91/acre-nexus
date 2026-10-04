import { describe, expect, it } from "vitest";
import { homeHref, isFieldCrew } from "./nav";

const has = (perms: string[]) => (p: string) => perms.includes(p);

describe("field crew", () => {
  const maintenance = [
    "property:read",
    "maintenance:read",
    "maintenance:manage",
    "calendar:read",
  ];
  it("is the maintenance role", () => {
    expect(isFieldCrew(has(maintenance))).toBe(true);
    expect(homeHref(has(maintenance))).toBe("/console/my-day");
  });
  it("is not anyone who also leases, bills or manages", () => {
    expect(isFieldCrew(has([...maintenance, "lease:manage"]))).toBe(false);
    expect(isFieldCrew(has([...maintenance, "ledger:read"]))).toBe(false);
    expect(isFieldCrew(has(["property:read", "listing:read"]))).toBe(false);
    expect(homeHref(has(["property:read", "lease:read"]))).toBe("/console");
  });
});
