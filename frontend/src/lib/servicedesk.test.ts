import { describe, expect, it } from "vitest";
import { minutesLabel, money, parseCents, tradeLabel } from "./servicedesk";

describe("service desk helpers", () => {
  it("labels trades and times", () => {
    expect(tradeLabel("hvac")).toBe("HVAC");
    expect(tradeLabel("plumbing")).toBe("Plumbing");
    expect(minutesLabel(90)).toBe("1h 30m");
    expect(minutesLabel(120)).toBe("2h");
    expect(minutesLabel(45)).toBe("45m");
    expect(minutesLabel(null)).toBe("");
  });
  it("shows spent money exactly", () => {
    expect(money(21455)).toBe("$214.55");
    expect(money(14900)).toBe("$149");
  });
  it("reads money typed by people", () => {
    expect(parseCents("$214.55")).toBe(21455);
    expect(parseCents("1,000")).toBe(100000);
    expect(parseCents("")).toBeNull();
    expect(parseCents("lots")).toBeNull();
    expect(parseCents("-5")).toBeNull();
  });
});
