import { describe, expect, it } from "vitest";
import { certTone, incomeLimit, qualifies } from "./family";

describe("income limits", () => {
  it("is a percent of AMI", () => {
    expect(incomeLimit(8_000_000, 60)).toBe(4_800_000);
    expect(qualifies(4_800_000, 8_000_000, 60)).toBe(true);
    expect(qualifies(4_800_001, 8_000_000, 60)).toBe(false);
  });
  it("colors the certification state", () => {
    expect(certTone("ok")).toBe("good");
    expect(certTone("expiring")).toBe("warn");
    expect(certTone("over_limit")).toBe("bad");
  });
});
