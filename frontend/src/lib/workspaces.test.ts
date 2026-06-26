import { describe, expect, it } from "vitest";
import { activeMembership, activeWorkspace } from "./workspaces";
import type { Membership, User, Workspace } from "./types";

const hq: Workspace = {
  kind: "platform",
  tenant_id: null,
  slug: null,
  name: "Acre HQ",
};
const northwind: Workspace = {
  kind: "tenant",
  tenant_id: "t-nw",
  slug: "northwind",
  name: "Northwind",
};

function user(over: Partial<User>): User {
  return {
    id: "u1",
    email: "a@b.com",
    name: "A B",
    tenant_id: null,
    is_platform_staff: true,
    permissions: [],
    active_tenant_id: null,
    memberships: [],
    workspaces: [hq, northwind],
    ...over,
  };
}

describe("activeWorkspace", () => {
  it("resolves the platform workspace when active_tenant_id is null", () => {
    expect(activeWorkspace(user({ active_tenant_id: null }))).toBe(hq);
  });

  it("resolves the matching tenant workspace when a tenant is active", () => {
    expect(activeWorkspace(user({ active_tenant_id: "t-nw" }))).toBe(northwind);
  });

  it("returns null for a null user", () => {
    expect(activeWorkspace(null)).toBeNull();
  });

  it("returns null when no workspace matches", () => {
    expect(activeWorkspace(user({ active_tenant_id: "missing" }))).toBeNull();
  });
});

describe("activeMembership", () => {
  const platformMember: Membership = {
    scope: "platform",
    tenant_id: null,
    tenant_slug: null,
    tenant_name: null,
    profile_type: "platform_admin",
    title: "Founder",
    status: "active",
    is_primary: true,
  };
  const tenantMember: Membership = {
    scope: "tenant",
    tenant_id: "t-nw",
    tenant_slug: "northwind",
    tenant_name: "Northwind",
    profile_type: "property_manager",
    title: null,
    status: "active",
    is_primary: true,
  };

  it("picks a platform membership when active is null", () => {
    expect(
      activeMembership(
        user({
          active_tenant_id: null,
          memberships: [platformMember, tenantMember],
        })
      )
    ).toBe(platformMember);
  });

  it("picks the membership matching the active tenant", () => {
    expect(
      activeMembership(
        user({
          active_tenant_id: "t-nw",
          memberships: [platformMember, tenantMember],
        })
      )
    ).toBe(tenantMember);
  });

  it("returns null when no membership matches", () => {
    expect(activeMembership(user({ memberships: [] }))).toBeNull();
  });
});
