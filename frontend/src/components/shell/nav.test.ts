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

import { offerFor } from "./nav";

describe("what a company is offered", () => {
  it("offers everything to a company with no properties yet", () => {
    expect(offerFor([], false)).toEqual({
      rentals: true,
      sites: true,
      layouts: true,
      foundation: true,
    });
  });
  it("gives a company of campgrounds bookings and no leases", () => {
    const o = offerFor(["campground", "rv_park"], false);
    expect(o).toMatchObject({
      rentals: false,
      sites: true,
      layouts: true,
      foundation: false,
    });
  });
  it("gives a company of houses leases and no campgrounds or layouts", () => {
    const o = offerFor(["single_family", "townhome"], false);
    expect(o).toMatchObject({ rentals: true, sites: false, layouts: false });
  });
  it("gives a mixed company all of it, and foundation only when one runs", () => {
    expect(offerFor(["multi_family", "campground"], true)).toEqual({
      rentals: true,
      sites: true,
      layouts: true,
      foundation: true,
    });
  });
});
