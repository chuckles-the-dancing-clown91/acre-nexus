import { describe, expect, it } from "vitest";
import { MODULES, defaultEnablement, moduleByKey } from "./registry";

describe("module registry", () => {
  it("looks up a module by key", () => {
    const mod = moduleByKey("properties");
    expect(mod).toBeDefined();
    expect(mod?.label).toBe("Property Management");
  });

  it("returns undefined for an unknown key", () => {
    expect(moduleByKey("does-not-exist")).toBeUndefined();
  });

  it("defaultEnablement covers every module key", () => {
    const map = defaultEnablement();
    expect(Object.keys(map).sort()).toEqual(MODULES.map((m) => m.key).sort());
  });

  it("acquisitions (flips) is GA and on by default", () => {
    const map = defaultEnablement();
    expect(map.flips).toBe(true);
    expect(map.properties).toBe(true);
    expect(moduleByKey("flips")?.preview).toBeFalsy();
  });

  it("any preview module defaults to disabled", () => {
    const map = defaultEnablement();
    for (const m of MODULES) {
      if (m.preview) expect(map[m.key]).toBe(false);
    }
  });
});
