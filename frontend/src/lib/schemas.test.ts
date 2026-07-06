import { describe, expect, it } from "vitest";
import {
  applicationSchema,
  createPayableSchema,
  createReminderSchema,
  createTokenSchema,
} from "./schemas";

describe("createTokenSchema", () => {
  it("accepts a valid token", () => {
    const res = createTokenSchema.safeParse({
      name: "Zillow sync",
      scopes: ["listing:read"],
    });
    expect(res.success).toBe(true);
  });

  it("rejects a too-short name", () => {
    const res = createTokenSchema.safeParse({
      name: "x",
      scopes: ["listing:read"],
    });
    expect(res.success).toBe(false);
    if (!res.success) {
      expect(res.error.issues[0].path).toContain("name");
    }
  });

  it("requires at least one scope", () => {
    const res = createTokenSchema.safeParse({ name: "Valid name", scopes: [] });
    expect(res.success).toBe(false);
  });

  it("rejects an unknown scope", () => {
    const res = createTokenSchema.safeParse({
      name: "Valid name",
      scopes: ["not:a:scope"],
    });
    expect(res.success).toBe(false);
  });
});

describe("applicationSchema", () => {
  it("accepts a minimal valid application", () => {
    const res = applicationSchema.safeParse({
      applicant_name: "Jane Doe",
      email: "jane@example.com",
    });
    expect(res.success).toBe(true);
  });

  it("rejects an invalid email", () => {
    const res = applicationSchema.safeParse({
      applicant_name: "Jane Doe",
      email: "not-an-email",
    });
    expect(res.success).toBe(false);
  });
});

describe("createPayableSchema", () => {
  const valid = {
    counterparty_id: "b8f7e9a0-0000-0000-0000-000000000001",
    memo: "HVAC repair — unit 2B",
    amount: 450.5,
  };

  it("accepts a minimal valid bill", () => {
    expect(createPayableSchema.safeParse(valid).success).toBe(true);
  });

  it("requires a vendor and a positive amount", () => {
    expect(
      createPayableSchema.safeParse({ ...valid, counterparty_id: "" }).success
    ).toBe(false);
    expect(createPayableSchema.safeParse({ ...valid, amount: 0 }).success).toBe(
      false
    );
    expect(
      createPayableSchema.safeParse({ ...valid, amount: -5 }).success
    ).toBe(false);
  });

  it("coerces a string amount from the number input", () => {
    const res = createPayableSchema.safeParse({ ...valid, amount: "125.75" });
    expect(res.success).toBe(true);
    if (res.success) {
      expect(res.data.amount).toBeCloseTo(125.75);
    }
  });
});

describe("createReminderSchema", () => {
  const valid = {
    subject_type: "license",
    title: "Rental license renewal",
    due_date: "2026-09-30",
  };

  it("accepts a minimal valid reminder", () => {
    expect(createReminderSchema.safeParse(valid).success).toBe(true);
  });

  it("rejects unknown subjects and malformed dates", () => {
    expect(
      createReminderSchema.safeParse({ ...valid, subject_type: "birthday" })
        .success
    ).toBe(false);
    expect(
      createReminderSchema.safeParse({ ...valid, due_date: "Sept 30" }).success
    ).toBe(false);
  });

  it("requires a title", () => {
    expect(
      createReminderSchema.safeParse({ ...valid, title: "" }).success
    ).toBe(false);
  });
});
