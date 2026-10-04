import { describe, expect, it } from "vitest";
import { needsReason, nextStatuses } from "./ticketFlow";

describe("work order status flow", () => {
  it("offers open work every move but straight to closed", () => {
    const n = nextStatuses("in_progress");
    expect(n).toContain("resolved");
    expect(n).toContain("on_hold");
    expect(n).not.toContain("closed");
    expect(n).not.toContain("in_progress");
  });
  it("closes only after resolving, and reopens from finished", () => {
    expect(nextStatuses("resolved")).toContain("closed");
    expect(nextStatuses("closed")).toEqual(["open", "in_progress"]);
  });
  it("asks why for cancelling and reopening, not for closing a resolved one", () => {
    expect(needsReason("open", "cancelled")).toBe(true);
    expect(needsReason("closed", "in_progress")).toBe(true);
    expect(needsReason("resolved", "closed")).toBe(false);
    expect(needsReason("open", "triage")).toBe(false);
  });
});
