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
  usePermissionsCatalog: () => ({ data: mocks.permissions, isLoading: false }),
  useCreateRole: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useUpdateRole: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useDeleteRole: () => ({ mutateAsync: vi.fn(), isPending: false }),
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

  it("lists system roles and flags them as System", () => {
    render(<RolesPage />);
    expect(screen.getByText("Platform admin")).toBeInTheDocument();
    // "System" appears at least once (the stat tile + the row's Type badge).
    expect(screen.getAllByText("System").length).toBeGreaterThanOrEqual(1);
  });

  it("opens a locked editor for a system role (no save, disabled checkboxes)", async () => {
    render(<RolesPage />);
    // The role list rows are clickable (DataTable onRowClick).
    await userEvent.click(screen.getByText("Platform admin"));
    // Locked: a "System · locked" badge appears.
    expect(screen.getByText(/System · locked/i)).toBeInTheDocument();
    // The permission checkbox is rendered and disabled.
    const checkbox = screen.getByRole("checkbox");
    expect(checkbox).toBeDisabled();
    // No "Save permissions" action for a locked role.
    expect(
      screen.queryByRole("button", { name: /Save permissions/i })
    ).not.toBeInTheDocument();
  });
});
