import { describe, expect, it } from "vitest";
import { dayOf, instantFrom, statusWords, weekOf, ymd } from "./appointments";

describe("appointments helpers", () => {
  it("reads status in plain words", () => {
    expect(statusWords("proposed")).toBe("Waiting on the resident");
    expect(statusWords("proposed", "vendor")).toBe("Waiting on the vendor");
    expect(statusWords("no_show")).toBe("Nobody home");
  });

  it("turns date and time inputs into an instant", () => {
    expect(instantFrom("2026-10-07", "09:00")).toBe("2026-10-07T09:00");
    expect(instantFrom("", "09:00")).toBeNull();
    expect(instantFrom("2026-13-40", "09:00")).toBeNull();
  });

  it("builds a Monday-first week", () => {
    const week = weekOf(new Date(2026, 9, 7)); // a Wednesday
    expect(week).toHaveLength(7);
    expect(week[0].getDay()).toBe(1);
    expect(ymd(week[0])).toBe("2026-10-05");
    expect(ymd(week[6])).toBe("2026-10-11");
    const sunday = weekOf(new Date(2026, 9, 11));
    expect(ymd(sunday[0])).toBe("2026-10-05");
  });

  it("places an appointment on its day", () => {
    const base = {
      starts_at: null,
      windows: [{ start: new Date(2026, 9, 8, 9).toISOString(), end: "" }],
    } as never;
    expect(dayOf(base)).toBe("2026-10-08");
    expect(dayOf({ starts_at: null, windows: [] } as never)).toBeNull();
  });
});
