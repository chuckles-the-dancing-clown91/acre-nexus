import { describe, expect, it } from "vitest";
import { demoAccountsEnabled, socialButtons } from "./signin";

describe("socialButtons", () => {
  it("shows nothing until the server reports providers", () => {
    expect(socialButtons(undefined)).toEqual([]);
    expect(socialButtons([])).toEqual([]);
  });

  it("shows only the providers the server offers", () => {
    expect(
      socialButtons([
        { key: "google", sandbox: false },
        { key: "myspace", sandbox: false },
      ])
    ).toEqual([{ key: "google", label: "Google" }]);
  });
});

describe("demoAccountsEnabled", () => {
  it("is hidden by default", () => {
    expect(demoAccountsEnabled(undefined)).toBe(false);
    expect(demoAccountsEnabled("")).toBe(false);
    expect(demoAccountsEnabled("0")).toBe(false);
    expect(demoAccountsEnabled("production")).toBe(false);
  });

  it("shows only when the build flag is on", () => {
    expect(demoAccountsEnabled("1")).toBe(true);
    expect(demoAccountsEnabled("true")).toBe(true);
  });
});
