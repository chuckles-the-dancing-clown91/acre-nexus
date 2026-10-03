import { describe, expect, it } from "vitest";
import {
  draftFrom,
  inQueueView,
  loadLabel,
  roleLabel,
  draftToReq,
  draftTotals,
  emptyDraft,
  looksLikeUrl,
  minutesLabel,
  money,
  parseCents,
  partLinks,
  tradeLabel,
  type Kit,
} from "./servicedesk";

describe("service desk helpers", () => {
  it("labels trades and times", () => {
    expect(tradeLabel("hvac")).toBe("HVAC");
    expect(tradeLabel("plumbing")).toBe("Plumbing");
    expect(minutesLabel(90)).toBe("1h 30m");
    expect(minutesLabel(120)).toBe("2h");
    expect(minutesLabel(45)).toBe("45m");
    expect(minutesLabel(null)).toBe("");
  });
  it("shows spent money exactly", () => {
    expect(money(21455)).toBe("$214.55");
    expect(money(14900)).toBe("$149");
  });
  it("reads money typed by people", () => {
    expect(parseCents("$214.55")).toBe(21455);
    expect(parseCents("1,000")).toBe(100000);
    expect(parseCents("")).toBeNull();
    expect(parseCents("lots")).toBeNull();
    expect(parseCents("-5")).toBeNull();
  });
});

const legacy: Kit = {
  id: "k1",
  name: "Running toilet",
  area: "Bathroom",
  category: "plumbing",
  priority: "normal",
  description: null,
  est_minutes: 30,
  checklist: ["Check flapper", "Replace part"],
  parts: [
    {
      name: "Flapper",
      quantity: 1,
      inventory_item_id: "inv1",
      unit_cost_cents: 899,
    },
  ],
  tasks: [],
  trades: [],
  contractor_trades: [],
  est_labor_cents: 0,
  est_parts_cents: 899,
  est_total_cents: 899,
  est_total_label: "$9",
  active: true,
};

describe("kit drafts", () => {
  it("turns an old checklist into tasks and keeps stock links", () => {
    const d = draftFrom(legacy);
    expect(d.tasks.map((t) => t.title)).toEqual([
      "Check flapper",
      "Replace part",
    ]);
    expect(d.parts[0]).toMatchObject({
      cost: "8.99",
      inventory_item_id: "inv1",
    });
    const r = draftToReq(d);
    expect(r.checklist).toEqual([]);
    expect(r.parts[0].unit_cost_cents).toBe(899);
    expect(r.parts[0].inventory_item_id).toBe("inv1");
  });
  it("drops blank lines and sums time", () => {
    const d = emptyDraft();
    d.name = "  Shower ";
    d.tasks = [
      { title: "Demo", trade: "demo", minutes: "120", needs_contractor: false },
      {
        title: "Valve",
        trade: "plumbing",
        minutes: "90",
        needs_contractor: true,
      },
      { title: "  ", trade: "general", minutes: "30", needs_contractor: false },
    ];
    d.parts = [
      {
        name: "Valve",
        quantity: "2",
        cost: "$150",
        url: " https://www.homedepot.com/p/valve/1 ",
        inventory_item_id: null,
      },
      {
        name: "Caulk",
        quantity: "",
        cost: "",
        url: "",
        inventory_item_id: null,
      },
      { name: "", quantity: "1", cost: "5", url: "", inventory_item_id: null },
    ];
    const r = draftToReq(d);
    expect(r.name).toBe("Shower");
    expect(r.tasks).toHaveLength(2);
    expect(r.est_minutes).toBe(210);
    expect(r.parts).toEqual([
      {
        name: "Valve",
        quantity: 2,
        inventory_item_id: null,
        unit_cost_cents: 15000,
        url: "https://www.homedepot.com/p/valve/1",
      },
      {
        name: "Caulk",
        quantity: 1,
        inventory_item_id: null,
        unit_cost_cents: null,
        url: null,
      },
    ]);
    expect(draftTotals(d)).toEqual({
      minutes: 120,
      contractorMinutes: 90,
      partsCents: 30000,
    });
  });
});

describe("partLinks", () => {
  it("uses the saved link and names its store", () => {
    expect(
      partLinks({
        name: "Dishwasher",
        url: "https://www.homedepot.com/p/1",
        store: "Home Depot",
      })
    ).toEqual([
      { label: "Buy at Home Depot", href: "https://www.homedepot.com/p/1" },
    ]);
  });

  it("falls back to store searches", () => {
    const links = partLinks({ name: "Wax ring & bolts" });
    expect(links.map((l) => l.label)).toEqual([
      "Home Depot",
      "Lowe's",
      "Amazon",
    ]);
    expect(links[2].href).toBe(
      "https://www.amazon.com/s?k=Wax%20ring%20%26%20bolts"
    );
  });
});

describe("looksLikeUrl", () => {
  it("takes web links and blanks, refuses the rest", () => {
    expect(looksLikeUrl("")).toBe(true);
    expect(looksLikeUrl("https://amzn.to/abc")).toBe(true);
    expect(looksLikeUrl("javascript:alert(1)")).toBe(false);
    expect(looksLikeUrl("homedepot.com/p/1")).toBe(false);
    expect(looksLikeUrl("https://x.com/a b")).toBe(false);
  });
});

describe("queue views", () => {
  const t = (over: Partial<Parameters<typeof inQueueView>[0]> = {}) => ({
    status: "open",
    priority: "normal",
    waiting_on: null,
    assignee_user_id: null,
    assignee_entity_id: null,
    ...over,
  });
  it("splits work by who has it and how it stands", () => {
    expect(inQueueView(t(), "unassigned", "me")).toBe(true);
    expect(inQueueView(t({ assignee_user_id: "me" }), "mine", "me")).toBe(true);
    expect(inQueueView(t({ assignee_user_id: "you" }), "mine", "me")).toBe(
      false
    );
    expect(inQueueView(t({ assignee_user_id: "me" }), "unassigned", "me")).toBe(
      false
    );
    expect(
      inQueueView(t({ assignee_entity_id: "v" }), "unassigned", "me")
    ).toBe(false);
    expect(inQueueView(t(), "mine", null)).toBe(false);
  });
  it("keeps closed work out of the open views", () => {
    expect(inQueueView(t({ status: "resolved" }), "open", "me")).toBe(false);
    expect(inQueueView(t({ status: "resolved" }), "done", "me")).toBe(true);
    expect(inQueueView(t({ priority: "high" }), "urgent", "me")).toBe(true);
    expect(inQueueView(t({ status: "on_hold" }), "waiting", "me")).toBe(true);
  });
  it("says what a teammate has", () => {
    expect(loadLabel({ open_tickets: 0, open_tasks: 0 })).toBe("free");
    expect(loadLabel({ open_tickets: 1, open_tasks: 3 })).toBe(
      "1 work order, 3 tasks"
    );
    expect(roleLabel("property_manager")).toBe("Property manager");
  });
});
