import { describe, expect, it } from "vitest";
import {
  compactUsd,
  grossYieldPct,
  monthlyPicture,
  perSqft,
} from "./propertyMath";
import { splitFeature } from "./propertyRecords";

describe("monthlyPicture", () => {
  it("nets rent against the loan, taxes and insurance", () => {
    const m = monthlyPicture({
      rentMonthly: 1_480_000,
      loanMonthly: 520_000,
      taxYearly: 1_962_400,
      insuranceYearly: 380_000,
    });
    expect(m.taxes).toBe(163_533);
    expect(m.insurance).toBe(31_667);
    expect(m.net).toBe(1_480_000 - 520_000 - 163_533 - 31_667);
  });
  it("counts what's missing as nothing", () => {
    expect(monthlyPicture({}).net).toBe(0);
    expect(monthlyPicture({ rentMonthly: 100 }).net).toBe(100);
  });
});

describe("yield and price per foot", () => {
  it("reads a year's rent against value", () => {
    expect(grossYieldPct(1_480_000, 219_780_000)).toBe(8.1);
    expect(grossYieldPct(null, 100)).toBeNull();
    expect(grossYieldPct(100, 0)).toBeNull();
  });
  it("divides value by area", () => {
    expect(perSqft(219_780_000, 2805)).toBe(784);
    expect(perSqft(100, 0)).toBeNull();
  });
});

describe("compactUsd", () => {
  it("shortens big amounts", () => {
    expect(compactUsd(219_780_000)).toBe("$2.2M");
    expect(compactUsd(1_480_000)).toBe("$15K");
    expect(compactUsd(185_000)).toBe("$1.9K");
    expect(compactUsd(95_000)).toBe("$950");
  });
});

describe("splitFeature", () => {
  it("separates a label from its value", () => {
    expect(splitFeature("Flooring: Hardwood")).toEqual({
      label: "Flooring",
      value: "Hardwood",
    });
    expect(splitFeature("Gas fireplace")).toEqual({
      label: null,
      value: "Gas fireplace",
    });
  });
});
