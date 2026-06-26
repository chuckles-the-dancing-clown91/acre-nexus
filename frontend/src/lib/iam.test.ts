import { describe, expect, it } from "vitest";
import { groupPermissions, humanizeKey } from "./iam";
import type { PermissionDef } from "./api";

function perm(key: string, category: string): PermissionDef {
  return { key, category, label: key, description: "", scope: "platform" };
}

describe("groupPermissions", () => {
  it("groups permissions by category in first-seen order", () => {
    const groups = groupPermissions([
      perm("user:read", "Users"),
      perm("role:read", "Roles"),
      perm("user:manage", "Users"),
    ]);
    expect(groups.map((g) => g.category)).toEqual(["Users", "Roles"]);
    expect(groups[0].permissions.map((p) => p.key)).toEqual([
      "user:read",
      "user:manage",
    ]);
    expect(groups[1].permissions.map((p) => p.key)).toEqual(["role:read"]);
  });

  it("returns an empty array for no permissions", () => {
    expect(groupPermissions([])).toEqual([]);
  });

  it("preserves order within a category", () => {
    const groups = groupPermissions([
      perm("a", "X"),
      perm("b", "X"),
      perm("c", "X"),
    ]);
    expect(groups).toHaveLength(1);
    expect(groups[0].permissions.map((p) => p.key)).toEqual(["a", "b", "c"]);
  });
});

describe("humanizeKey", () => {
  it("turns a snake-case key into a sentence-cased label", () => {
    expect(humanizeKey("platform_admin")).toBe("Platform admin");
    expect(humanizeKey("regional-manager")).toBe("Regional manager");
  });
});
