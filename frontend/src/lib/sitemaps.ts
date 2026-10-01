// Site maps: a property laid out on a map (roadmap area 1).

import { DEFAULT_TENANT, request } from "@/lib/api";

export type Geometry =
  | { type: "Point"; coordinates: [number, number] }
  | { type: "LineString"; coordinates: [number, number][] }
  | { type: "Polygon"; coordinates: [number, number][][] }
  | { type: string; coordinates: unknown };

export type FeatureKind =
  | "building"
  | "unit"
  | "site"
  | "amenity"
  | "road"
  | "boundary"
  | "parking"
  | "water"
  | "label";

export interface UnitInfo {
  unit_number: string;
  status: string;
  market_rent_label: string | null;
  beds: number | null;
  baths: number | null;
}

export interface Feature {
  /** Server id; absent until the first save. */
  id?: string;
  /** Stable client key for unsaved features. */
  key: string;
  kind: FeatureKind;
  name: string | null;
  geometry: Geometry;
  unit_id: string | null;
  attrs: Record<string, unknown>;
  area_m2?: number | null;
  length_m?: number | null;
  unit?: UnitInfo | null;
}

export interface SiteMap {
  id: string;
  property_id: string;
  property_name: string;
  name: string;
  kind: "apartment" | "campground" | "rv_park" | "other";
  base_layer: "satellite" | "streets" | "plan" | "grid";
  center_lng: number | null;
  center_lat: number | null;
  zoom: number;
  plan_document_id: string | null;
  plan_corners: [number, number][] | null;
  published: boolean;
  notes: string | null;
  updated_at: string;
  stats: {
    features: number;
    units: number;
    units_available: number;
    sites: number;
    sites_available: number;
  };
  features: (Omit<Feature, "key"> & { id: string })[] | null;
}

export const KIND_LABELS: Record<FeatureKind, string> = {
  building: "Building",
  unit: "Unit",
  site: "Campsite",
  amenity: "Amenity",
  road: "Road",
  boundary: "Boundary",
  parking: "Parking",
  water: "Water",
  label: "Label",
};

/** Which shape each kind is drawn with. */
export const DRAW_SHAPE: Record<FeatureKind, "point" | "polygon" | "line"> = {
  building: "polygon",
  unit: "polygon",
  site: "point",
  amenity: "point",
  road: "line",
  boundary: "polygon",
  parking: "polygon",
  water: "polygon",
  label: "point",
};

export const KIND_COLORS: Record<FeatureKind, string> = {
  building: "#64748b",
  unit: "#0e7c86",
  site: "#16a34a",
  amenity: "#7c3aed",
  road: "#a8a29e",
  boundary: "#f59e0b",
  parking: "#94a3b8",
  water: "#3b82f6",
  label: "#111827",
};

export const UNIT_STATUS_COLORS: Record<string, string> = {
  vacant: "#16a34a",
  available: "#16a34a",
  occupied: "#0e7c86",
  notice: "#f59e0b",
  make_ready: "#f97316",
  down: "#dc2626",
  unavailable: "#9ca3af",
};

export const SITE_STATUS_COLORS: Record<string, string> = {
  available: "#16a34a",
  booked: "#0e7c86",
  blocked: "#f59e0b",
  out_of_service: "#dc2626",
};

export const SITE_TYPES = ["tent", "rv", "cabin", "glamping", "group", "other"];
export const AMENITIES = [
  "bath_house",
  "dump_station",
  "laundry",
  "fire_ring",
  "playground",
  "office",
  "pool",
  "trash",
  "water_spigot",
  "other",
];

/** The colour a feature is drawn with. */
export function featureColor(f: Feature): string {
  if (f.kind === "unit" && f.unit) {
    return UNIT_STATUS_COLORS[f.unit.status] ?? KIND_COLORS.unit;
  }
  if (f.kind === "site") {
    return (
      SITE_STATUS_COLORS[String(f.attrs.site_status ?? "available")] ??
      KIND_COLORS.site
    );
  }
  return KIND_COLORS[f.kind];
}

let counter = 0;
export const newKey = () => `n${Date.now().toString(36)}${counter++}`;

export function fromServer(m: SiteMap): Feature[] {
  return (m.features ?? []).map((f) => ({ ...f, key: f.id }));
}

const post = <T>(path: string, body: unknown = {}) =>
  request<T>(path, { method: "POST", auth: true, body });

export const siteMaps = {
  list: (property_id?: string) =>
    request<SiteMap[]>(
      `/site-maps${property_id ? `?property_id=${property_id}` : ""}`,
      { auth: true }
    ),
  get: (id: string) => request<SiteMap>(`/site-maps/${id}`, { auth: true }),
  create: (body: {
    property_id: string;
    name: string;
    kind: string;
    base_layer: string;
    center_lng?: number;
    center_lat?: number;
  }) => post<SiteMap>("/site-maps", body),
  update: (id: string, body: Record<string, unknown>) =>
    request<SiteMap>(`/site-maps/${id}`, { method: "PATCH", auth: true, body }),
  remove: (id: string) =>
    request<{ ok: boolean }>(`/site-maps/${id}`, {
      method: "DELETE",
      auth: true,
    }),
  saveFeatures: (id: string, features: Feature[]) =>
    request<SiteMap>(`/site-maps/${id}/features`, {
      method: "PUT",
      auth: true,
      body: {
        features: features.map((f) => ({
          id: f.id,
          kind: f.kind,
          name: f.name,
          geometry: f.geometry,
          unit_id: f.unit_id,
          attrs: f.attrs,
        })),
      },
    }),
  exportGeoJson: (id: string) =>
    request<unknown>(`/site-maps/${id}/export.geojson`, { auth: true }),
  importGeoJson: (id: string, geojson: unknown, replace: boolean) =>
    post<SiteMap>(`/site-maps/${id}/import`, { geojson, replace }),
  plan: (id: string) =>
    request<{ url: string }>(`/site-maps/${id}/plan`, { auth: true }),
  publicList: (property_id?: string) =>
    request<SiteMap[]>(
      `/public/site-maps${property_id ? `?property_id=${property_id}` : ""}`,
      { tenant: DEFAULT_TENANT }
    ),
  publicGet: (id: string, tenant: string = DEFAULT_TENANT) =>
    request<SiteMap>(`/public/site-maps/${id}`, { tenant }),
  publicPlan: (id: string, tenant: string = DEFAULT_TENANT) =>
    request<{ url: string }>(`/public/site-maps/${id}/plan`, { tenant }),
};

/** Four corners for a plan image: `widthM` wide, rotated, centred on a point. */
export function planCorners(
  center: [number, number],
  imgW: number,
  imgH: number,
  widthM: number,
  rotationDeg: number
): [number, number][] {
  const [lng, lat] = center;
  const kx = 111320 * Math.cos((lat * Math.PI) / 180);
  const ky = 111320;
  const w = widthM;
  const h = (widthM * imgH) / imgW;
  const r = (rotationDeg * Math.PI) / 180;
  const pts: [number, number][] = [
    [-w / 2, h / 2],
    [w / 2, h / 2],
    [w / 2, -h / 2],
    [-w / 2, -h / 2],
  ];
  return pts.map(([x, y]) => {
    const rx = x * Math.cos(r) - y * Math.sin(r);
    const ry = x * Math.sin(r) + y * Math.cos(r);
    return [lng + rx / kx, lat + ry / ky];
  });
}

/** Copy a feature `count` times, each `step` metres east of the last. */
export function rowOf(
  f: Feature,
  count: number,
  stepM: number,
  nameFor: (i: number) => string | null
): Feature[] {
  const out: Feature[] = [];
  const lat0 =
    f.geometry.type === "Point"
      ? (f.geometry.coordinates as [number, number])[1]
      : ((f.geometry.coordinates as [number, number][][])[0]?.[0]?.[1] ?? 0);
  const dLng = stepM / (111320 * Math.cos((lat0 * Math.PI) / 180));
  const shift = (g: Geometry, d: number): Geometry => {
    if (g.type === "Point") {
      const [x, y] = g.coordinates as [number, number];
      return { type: "Point", coordinates: [x + d, y] };
    }
    if (g.type === "LineString") {
      return {
        type: "LineString",
        coordinates: (g.coordinates as [number, number][]).map(([x, y]) => [
          x + d,
          y,
        ]),
      };
    }
    return {
      type: "Polygon",
      coordinates: (g.coordinates as [number, number][][]).map((ring) =>
        ring.map(([x, y]) => [x + d, y] as [number, number])
      ),
    };
  };
  for (let i = 1; i <= count; i++) {
    out.push({
      ...f,
      id: undefined,
      key: newKey(),
      unit_id: null,
      unit: null,
      name: nameFor(i),
      geometry: shift(f.geometry, dLng * i),
    });
  }
  return out;
}

/** "A1" then 2 → "A2"; "Site 9" then 1 → "Site 10". */
export function nextName(base: string | null, i: number): string | null {
  if (!base) return null;
  const m = base.match(/^(.*?)(\d+)$/);
  return m ? `${m[1]}${Number(m[2]) + i}` : `${base} ${i + 1}`;
}
