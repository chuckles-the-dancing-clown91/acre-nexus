import { describe, expect, it } from "vitest";
import { fitView, fitZoom, place, project, tilesFor } from "./tilemap";
import { pinTone, type MapPin } from "./analytics";

const portland = { lat: 45.5152, lng: -122.6784 };
const lakeOswego = { lat: 45.4207, lng: -122.6706 };

describe("tile math", () => {
  it("projects the origin to the middle of the world", () => {
    const p = project({ lat: 0, lng: 0 }, 0);
    expect(p.x).toBeCloseTo(128);
    expect(p.y).toBeCloseTo(128);
  });

  it("fits nearby points closer in than far ones", () => {
    const near = fitZoom([portland, lakeOswego], 800, 500);
    const far = fitZoom([portland, { lat: 40.71, lng: -74.0 }], 800, 500);
    expect(near).toBeGreaterThan(far);
    expect(near).toBeLessThanOrEqual(16);
  });

  it("puts every fitted point inside the box", () => {
    const pts = [portland, lakeOswego, { lat: 45.6, lng: -122.5 }];
    const v = fitView(pts, 800, 500);
    for (const p of pts) {
      const { left, top } = place(p, v);
      expect(left).toBeGreaterThanOrEqual(0);
      expect(left).toBeLessThanOrEqual(800);
      expect(top).toBeGreaterThanOrEqual(0);
      expect(top).toBeLessThanOrEqual(500);
    }
  });

  it("covers the box with tiles", () => {
    const v = fitView([portland, lakeOswego], 800, 500);
    const tiles = tilesFor(v, 800, 500);
    expect(tiles.length).toBeGreaterThanOrEqual(12);
    expect(Math.min(...tiles.map((t) => t.left))).toBeLessThanOrEqual(0);
    expect(Math.max(...tiles.map((t) => t.left + 256))).toBeGreaterThanOrEqual(
      800
    );
  });
});

describe("pin colours", () => {
  const pin = (o: Partial<MapPin>): MapPin => ({
    property_id: "p",
    name: "",
    address: "",
    city: "",
    state: "",
    lat: 0,
    lng: 0,
    units: 10,
    occupied: 10,
    occupancy_pct: 100,
    open_tickets: 0,
    urgent_tickets: 0,
    open_turns: 0,
    site_map_id: null,
    image_url: null,
    monthly_rent_cents: 0,
    ...o,
  });
  it("reads occupancy", () => {
    expect(pinTone(pin({}), "occupancy")).toBe("good");
    expect(pinTone(pin({ occupancy_pct: 90 }), "occupancy")).toBe("warn");
    expect(pinTone(pin({ occupancy_pct: 50 }), "occupancy")).toBe("bad");
    expect(pinTone(pin({ occupancy_pct: null }), "occupancy")).toBe("neutral");
  });
  it("reads open work", () => {
    expect(pinTone(pin({}), "work")).toBe("good");
    expect(pinTone(pin({ open_turns: 1 }), "work")).toBe("warn");
    expect(pinTone(pin({ urgent_tickets: 1, open_tickets: 1 }), "work")).toBe(
      "bad"
    );
  });
});
