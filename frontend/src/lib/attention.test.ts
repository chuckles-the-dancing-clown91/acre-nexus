import { describe, expect, it } from "vitest";
import {
  cadenceWords,
  dueWords,
  groupByKind,
  type AttentionItem,
} from "./attention";

const item = (kind: AttentionItem["kind"], key: string): AttentionItem => ({
  kind,
  key,
  title: key,
  detail: "",
  property_id: null,
  property_name: null,
  href: "/",
  due_on: null,
  priority: "normal",
});

describe("attention words", () => {
  it("says when a routine is due", () => {
    expect(dueWords(-1)).toBe("1 day overdue");
    expect(dueWords(-3)).toBe("3 days overdue");
    expect(dueWords(0)).toBe("due today");
    expect(dueWords(1)).toBe("due in 1 day");
    expect(dueWords(12)).toBe("due in 12 days");
  });
  it("words a cadence", () => {
    expect(cadenceWords(365)).toBe("Yearly");
    expect(cadenceWords(182)).toBe("Every 6 months");
    expect(cadenceWords(730)).toBe("Every 2 years");
    expect(cadenceWords(2555)).toBe("Every 7 years");
    expect(cadenceWords(45)).toBe("Every 45 days");
  });
  it("groups by kind in page order, skipping empty kinds", () => {
    const g = groupByKind([
      item("profile", "a"),
      item("approval", "b"),
      item("profile", "c"),
    ]);
    expect(g.map(([k]) => k)).toEqual(["approval", "profile"]);
    expect(g[1][1].map((i) => i.key)).toEqual(["a", "c"]);
  });
});
