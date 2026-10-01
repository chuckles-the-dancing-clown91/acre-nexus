import { describe, expect, it } from "vitest";
import { MODULES } from "@/modules/registry";
import { ICONS } from "./icon";

describe("icon map", () => {
  it("has an icon for every module nav entry", () => {
    const missing = MODULES.flatMap((m) => m.nav)
      .filter((item) => !(item.icon in ICONS))
      .map((item) => `${item.href} → ${item.icon}`);
    expect(missing).toEqual([]);
  });
});
