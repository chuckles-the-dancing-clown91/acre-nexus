import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { PermissionDef, Role } from "@/lib/api";

// Mock the auth + query hooks so the page renders without a provider tree.
const mocks = vi.hoisted(() => ({
  can: vi.fn((perm: string) => perm === "role:read"),
  roles: [] as Role[],
  permissions: [] as PermissionDef[],
}));

vi.mock("@/lib/auth", () => ({
  useAuth: () => ({
    can: mocks.can,
    user: { is_platform_staff: true },
  }),
}));

vi.mock("@/lib/queries", () => ({
  useRoles: () => ({ data: mocks.roles, error: null, isLoading: false }),
  usePermissionsCatalog: () => ({ data: mocks.permissions }),
  useCreateRole: () => ({ mutateAsync: vi.fn() }),
  useUpdateRole: () => ({ mutate: vi.fn(), isPending: false }),
  useDeleteRole: () => ({ mutate: vi.fn(), isPending: false }),
}));

import RolesPage from "./page";

const systemRole: Role = {
  id: "r1",
  scope: "platform",
  tenant_id: null,
  key: "platform_admin",
  name: "Platform admin",
  description: "Full access",
  is_system: true,
  permissions: ["user:read"],
};

describe("RolesPage", () => {
  beforeEach(() => {
    mocks.can.mockImplementation((perm: string) => perm === "role:read");
    mocks.roles = [systemRole];
    mocks.permissions = [
      {
        key: "user:read",
        category: "Users",
        label: "Read users",
        description: "",
        scope: "platform",
      },
    ];
  });

  it("marks system roles as System in the list", () => {
    render(<RolesPage />);
    expect(screen.getByText("Platform admin")).toBeInTheDocument();
    expect(screen.getByText("System")).toBeInTheDocument();
  });

  it("renders the editor with disabled checkboxes for a system role", async () => {
    render(<RolesPage />);
    await userEvent.click(
      screen.getByRole("button", { name: /Platform admin/i })
    );
    // Locked: a "System · locked" badge appears and checkboxes are disabled.
    expect(screen.getByText(/System · locked/i)).toBeInTheDocument();
    const checkbox = screen.getByRole("checkbox");
    expect(checkbox).toBeDisabled();
    // No "Save permissions" action for a locked role.
    expect(
      screen.queryByRole("button", { name: /Save permissions/i })
    ).not.toBeInTheDocument();
  });
});
