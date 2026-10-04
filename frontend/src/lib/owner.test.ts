import { describe, expect, it } from "vitest";
import { approvalWords, isOwnerOnly, monthName, previousMonth } from "./owner";

describe("owner helpers", () => {
  it("knows an owner-only account", () => {
    const m = (p: string) => ({ profile_type: p, status: "active" });
    expect(
      isOwnerOnly({
        is_platform_staff: false,
        memberships: [m("landlord")],
      } as never)
    ).toBe(true);
    expect(
      isOwnerOnly({
        is_platform_staff: false,
        memberships: [m("landlord"), m("renter")],
      } as never)
    ).toBe(false);
    expect(
      isOwnerOnly({
        is_platform_staff: true,
        memberships: [m("landlord")],
      } as never)
    ).toBe(false);
    expect(isOwnerOnly(null)).toBe(false);
  });
  it("names months", () => {
    expect(monthName("2026-10")).toBe("October 2026");
    expect(previousMonth("2026-01")).toBe("2025-12");
    expect(previousMonth("2026-10")).toBe("2026-09");
  });
  it("words an answer", () => {
    expect(approvalWords({ kind: "approval", status: "pending" })).toBe(
      "Waiting on you"
    );
    expect(approvalWords({ kind: "signoff", status: "disputed" })).toBe(
      "Disputed"
    );
    expect(approvalWords({ kind: "approval", status: "overridden" })).toBe(
      "Went ahead (emergency)"
    );
  });
});
