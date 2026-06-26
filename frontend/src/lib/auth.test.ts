import { describe, expect, it } from "vitest";
import { hasPermission } from "./auth";

const base = {
  id: "u1",
  email: "a@b.com",
  name: "A B",
  tenant_id: "t1",
  is_platform_staff: false,
};

describe("hasPermission", () => {
  it("returns false for a null user", () => {
    expect(hasPermission(null, "property:read")).toBe(false);
  });

  it("grants when the exact permission is held", () => {
    const user = { ...base, permissions: ["property:read"] };
    expect(hasPermission(user, "property:read")).toBe(true);
  });

  it("denies when the permission is missing", () => {
    const user = { ...base, permissions: ["listing:read"] };
    expect(hasPermission(user, "property:read")).toBe(false);
  });

  it("platform:admin is a super-permission that grants everything", () => {
    const user = { ...base, permissions: ["platform:admin"] };
    expect(hasPermission(user, "property:read")).toBe(true);
    expect(hasPermission(user, "anything:at:all")).toBe(true);
  });
});
