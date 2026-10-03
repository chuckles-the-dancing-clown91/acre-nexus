import { describe, expect, it } from "vitest";
import {
  isResidentOnly,
  landingFor,
  prefillFrom,
  residentStatus,
} from "./resident";
import type { Membership } from "./types";

const m = (profile_type: string, status = "active"): Membership => ({
  scope: "tenant",
  tenant_id: "t",
  tenant_slug: "northwind",
  tenant_name: "Northwind",
  profile_type,
  title: null,
  status,
  is_primary: true,
});

describe("isResidentOnly", () => {
  it("is true for a renter and nobody else", () => {
    expect(
      isResidentOnly({ is_platform_staff: false, memberships: [m("renter")] })
    ).toBe(true);
    expect(
      isResidentOnly({
        is_platform_staff: false,
        memberships: [m("renter"), m("property_manager")],
      })
    ).toBe(false);
    expect(
      isResidentOnly({ is_platform_staff: true, memberships: [m("renter")] })
    ).toBe(false);
    expect(isResidentOnly({ is_platform_staff: false, memberships: [] })).toBe(
      false
    );
    expect(isResidentOnly(null)).toBe(false);
  });

  it("ignores a revoked staff role", () => {
    expect(
      isResidentOnly({
        is_platform_staff: false,
        memberships: [m("renter"), m("property_manager", "revoked")],
      })
    ).toBe(true);
  });
});

describe("residentStatus", () => {
  it("says what's happening in plain words", () => {
    expect(residentStatus("triage")).toBe("Received");
    expect(residentStatus("on_hold", "resident")).toBe("Waiting on you");
    expect(residentStatus("on_hold", "parts")).toBe("Waiting on parts");
    expect(residentStatus("resolved")).toBe("Done");
  });
});

describe("prefillFrom", () => {
  it("reads a text link's request", () => {
    const p = new URLSearchParams(
      "new=1&title=The%20kitchen%20faucet%20is%20leaking&category=plumbing"
    );
    expect(prefillFrom(p)).toEqual({
      title: "The kitchen faucet is leaking",
      category: "plumbing",
      description: "",
    });
    expect(prefillFrom(new URLSearchParams("ticket=1"))).toBeNull();
  });
});

describe("landingFor", () => {
  const renter = { is_platform_staff: false, memberships: [m("renter")] };
  const staff = {
    is_platform_staff: false,
    memberships: [m("property_manager")],
  };
  it("sends residents home and staff to the console", () => {
    expect(landingFor(renter, "")).toBe("/account/maintenance");
    expect(landingFor(staff, "")).toBe("/console");
  });
  it("follows a same-site next and nothing else", () => {
    expect(
      landingFor(renter, "?next=%2Faccount%2Fmaintenance%3Fticket%3D1")
    ).toBe("/account/maintenance?ticket=1");
    expect(landingFor(staff, "?next=https://evil.example")).toBe("/console");
    expect(landingFor(staff, "?next=//evil.example")).toBe("/console");
    expect(landingFor(staff, "?next=/%5Cevil.example")).toBe("/console");
  });
});
