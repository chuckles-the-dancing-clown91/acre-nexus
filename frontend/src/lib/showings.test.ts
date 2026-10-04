import { describe, expect, it } from "vitest";
import { dollarsToCents, leadStage, mapLink, phoneLinks } from "./showings";

describe("showings helpers", () => {
  it("builds call and text links", () => {
    expect(phoneLinks("(503) 555-0142")).toEqual({
      tel: "tel:5035550142",
      sms: "sms:5035550142",
    });
    expect(phoneLinks("")).toBeNull();
    expect(phoneLinks(null)).toBeNull();
  });
  it("links an address to a map", () => {
    expect(mapLink("700 Harbor Dr, Portland")).toContain("700%20Harbor");
  });
  it("words the lead stage", () => {
    expect(leadStage({ status: "toured" } as never)).toBe("Toured");
    expect(leadStage({ status: "odd" } as never)).toBe("odd");
  });
  it("reads dollars", () => {
    expect(dollarsToCents("1,850")).toBe(185000);
    expect(dollarsToCents("1850.5")).toBe(185050);
    expect(dollarsToCents("x")).toBeNull();
  });
});
