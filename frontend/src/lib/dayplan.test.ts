import { describe, expect, it } from "vitest";
import {
  dayRange,
  dollars,
  hours,
  stopsToAccept,
  storeSearch,
  type Stop,
} from "./dayplan";

const stop = (kind: Stop["kind"], id: string | null): Stop => ({
  kind,
  ticket_id: id,
  title: "x",
  property_id: null,
  property_name: "",
  address: "",
  priority: "normal",
  status: "open",
  assignee_user_id: null,
  fixed: false,
  minutes: 60,
  drive_minutes: 10,
  start: "2026-10-06T15:00:00Z",
  end: "2026-10-06T16:00:00Z",
  when_words: "",
  tasks_total: 0,
  tasks_done: 0,
  to_buy: 0,
  from_stock: 0,
  with_name: null,
  with_phone: null,
  access_notes: null,
  note: null,
});

describe("dayplan helpers", () => {
  it("words minutes", () => {
    expect(hours(45)).toBe("45m");
    expect(hours(120)).toBe("2h");
    expect(hours(90)).toBe("1h 30m");
  });
  it("words dollars", () => {
    expect(dollars(4200)).toBe("$42");
    expect(dollars(4250)).toBe("$42.50");
  });
  it("knows store search pages", () => {
    expect(storeSearch("Home Depot", "p-trap 1 1/2")).toContain(
      "homedepot.com/s/p-trap"
    );
    expect(storeSearch("Any store", "p-trap")).toBeNull();
  });
  it("sends only the jobs to accept", () => {
    const out = stopsToAccept([stop("store", null), stop("job", "t1")]);
    expect(out).toEqual([
      {
        ticket_id: "t1",
        start: "2026-10-06T15:00:00Z",
        end: "2026-10-06T16:00:00Z",
      },
    ]);
  });
  it("lists the days in a range", () => {
    expect(dayRange("2026-10-30", "2026-11-02")).toEqual([
      "2026-10-30",
      "2026-10-31",
      "2026-11-01",
      "2026-11-02",
    ]);
  });
});
