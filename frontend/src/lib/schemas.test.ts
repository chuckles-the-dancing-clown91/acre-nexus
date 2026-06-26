import { describe, expect, it } from "vitest";
import { applicationSchema, createTokenSchema } from "./schemas";

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
