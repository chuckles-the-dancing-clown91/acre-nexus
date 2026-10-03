import { describe, expect, it } from "vitest";
import { centsFrom, vendorResponseWords } from "./vendorLink";

describe("vendor link helpers", () => {
  it("reads dollar amounts into cents", () => {
    expect(centsFrom("385")).toBe(38500);
    expect(centsFrom("$1,234.5")).toBe(123450);
    expect(centsFrom("12.34")).toBe(1234);
    expect(centsFrom("abc")).toBeNull();
    expect(centsFrom("1.234")).toBeNull();
    expect(centsFrom("")).toBeNull();
  });

  it("words the vendor's answer", () => {
    expect(vendorResponseWords("accepted")).toBe("Accepted");
    expect(vendorResponseWords("done")).toBe("Vendor says done");
    expect(vendorResponseWords(null)).toBeNull();
  });
});
