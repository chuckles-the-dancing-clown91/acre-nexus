import { describe, expect, it } from "vitest";
import { madeSentence } from "./datatransfer";
import { altFromFilename, since } from "./syndication";

describe("import and syndication helpers", () => {
  it("says what an import made", () => {
    expect(madeSentence({ properties: 1, units: 3, leases: 2 })).toBe(
      "1 property, 3 units and 2 leases"
    );
    expect(madeSentence({ vendors: 1 })).toBe("1 vendor");
    expect(madeSentence({})).toBe("nothing new");
  });
  it("guesses alt text from a photo's filename", () => {
    expect(altFromFilename("living-room.jpg", "Photo")).toBe("Living room");
    expect(altFromFilename("IMG_2041.JPG", "Photo 1")).toBe("Photo 1");
    expect(altFromFilename("kitchen_after.png", "Photo")).toBe("Kitchen after");
  });
  it("says when a portal last came by", () => {
    const now = Date.parse("2026-10-03T12:00:00Z");
    expect(since("2026-10-03T11:59:40Z", now)).toBe("just now");
    expect(since("2026-10-03T11:15:00Z", now)).toBe("45 min ago");
    expect(since("2026-10-03T10:00:00Z", now)).toBe("2 hours ago");
    expect(since("2026-10-01T12:00:00Z", now)).toBe("2 days ago");
  });
});
