import { describe, expect, it } from "vitest";
import { compare, monthWords, trendWords } from "./Safety";

const o = (agency: number, state: number, prior: number | null) => ({
  key: "burglary",
  label: "Burglary",
  agency: 0,
  agency_rate: agency,
  state_rate: state,
  us_rate: 0,
  prior_rate: prior,
});

describe("safety words", () => {
  it("names the month", () => {
    expect(monthWords("01-2025")).toBe("Jan 2025");
    expect(monthWords("12-2024")).toBe("Dec 2024");
    expect(monthWords("x")).toBe("x");
  });
  it("compares with the state", () => {
    expect(compare(o(720, 324, null)).words).toBe("122% above the state");
    expect(compare(o(300, 324, null)).words).toBe("about the state rate");
    expect(compare(o(100, 324, null)).words).toBe("69% below the state");
    expect(compare(o(100, 0, null)).ratio).toBeNull();
  });
  it("words the trend", () => {
    expect(trendWords(o(110, 1, 100))).toBe("up 10% on the year before");
    expect(trendWords(o(90, 1, 100))).toBe("down 10% on the year before");
    expect(trendWords(o(101, 1, 100))).toBe("flat on the year before");
    expect(trendWords(o(101, 1, null))).toBeNull();
  });
});
