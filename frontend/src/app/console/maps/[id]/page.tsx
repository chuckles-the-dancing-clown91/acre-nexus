"use client";

// The site map editor: draw buildings and units or campsites, link units to
// records, set campsite details, import and export GeoJSON, publish.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import { toast } from "sonner";
import { api } from "@/lib/api";
import type { Unit } from "@/lib/types";
import {
  AMENITIES,
  DRAW_SHAPE,
  KIND_COLORS,
  KIND_LABELS,
  SITE_STATUS_COLORS,
  SITE_TYPES,
  UNIT_STATUS_COLORS,
  featureColor,
  fromServer,
  newKey,
  nextName,
  planCorners,
  rowOf,
  siteMaps,
  type Feature,
  type FeatureKind,
  type Geometry,
  type SiteMap,
} from "@/lib/sitemaps";
import { useAuth } from "@/lib/auth";
import { Badge, Button, Card, StatTile } from "@/components/ui";
import { SiteMapCanvas, type Tool } from "@/components/maps";

const field =
  "w-full rounded-lg border border-line bg-surface px-2 py-1.5 text-sm text-ink";

const APARTMENT_KINDS: FeatureKind[] = [
  "building",
  "unit",
  "amenity",
  "parking",
  "road",
  "boundary",
  "label",
];
const CAMP_KINDS: FeatureKind[] = [
  "site",
  "amenity",
  "road",
  "boundary",
  "parking",
  "water",
  "label",
];

const M2_TO_FT2 = 10.7639;
const M_TO_FT = 3.28084;

export default function MapEditorPage() {
  const { id } = useParams<{ id: string }>();
  const { can } = useAuth();
  const write = can("property:write");
  const [map, setMap] = useState<SiteMap | null>(null);
  const [features, setFeatures] = useState<Feature[]>([]);
  const [dirty, setDirty] = useState(false);
  const [units, setUnits] = useState<Unit[]>([]);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [tool, setTool] = useState<Tool>({ mode: "select" });
  const [drawKind, setDrawKind] = useState<FeatureKind>("unit");
  const [editKey, setEditKey] = useState<string | null>(null);
  const [planUrl, setPlanUrl] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const view = useRef<[number, number, number] | null>(null);

  const apply = useCallback((m: SiteMap, keepDirty = false) => {
    setMap(m);
    if (!keepDirty) {
      setFeatures(fromServer(m));
      setDirty(false);
    }
  }, []);

  useEffect(() => {
    siteMaps
      .get(id)
      .then((m) => {
        apply(m);
        setDrawKind(m.kind === "apartment" ? "unit" : "site");
        api
          .units(m.property_id)
          .then(setUnits)
          .catch(() => {});
        if (m.plan_document_id)
          siteMaps
            .plan(id)
            .then((p) => setPlanUrl(p.url))
            .catch(() => {});
      })
      .catch((e: Error) => setError(e.message));
  }, [id, apply]);

  // Leaving with unsaved drawing asks first.
  useEffect(() => {
    if (!dirty) return;
    const h = (e: BeforeUnloadEvent) => e.preventDefault();
    window.addEventListener("beforeunload", h);
    return () => window.removeEventListener("beforeunload", h);
  }, [dirty]);

  const selected = features.find((f) => f.key === selectedKey) ?? null;
  const linked = useMemo(
    () => new Set(features.map((f) => f.unit_id).filter(Boolean)),
    [features]
  );

  const patch = (key: string, p: Partial<Feature>) => {
    setFeatures((fs) => fs.map((f) => (f.key === key ? { ...f, ...p } : f)));
    setDirty(true);
  };
  const setAttr = (key: string, name: string, value: unknown) => {
    const f = features.find((x) => x.key === key);
    if (!f) return;
    const attrs = { ...f.attrs };
    if (value === "" || value === undefined || value === null)
      delete attrs[name];
    else attrs[name] = value;
    patch(key, { attrs });
  };

  const onDrawn = (geometry: Geometry) => {
    const kind = drawKind;
    const same = features.filter((f) => f.kind === kind && f.name);
    const last = same[same.length - 1]?.name ?? null;
    const name =
      kind === "site" || kind === "unit"
        ? (nextName(last, 0) ?? (kind === "site" ? "Site 1" : null))
        : null;
    const f: Feature = {
      key: newKey(),
      kind,
      name,
      geometry,
      unit_id: null,
      attrs:
        kind === "site"
          ? { site_type: "tent", site_status: "available" }
          : kind === "amenity"
            ? { amenity: "bath_house" }
            : {},
    };
    setFeatures((fs) => [...fs, f]);
    setDirty(true);
    setSelectedKey(f.key);
    // Points (campsites, amenities) keep drawing; shapes return to select.
    if (DRAW_SHAPE[kind] !== "point") setTool({ mode: "select" });
  };

  const onEdited = (key: string, geometry: Geometry) =>
    patch(key, { geometry });

  const save = async () => {
    if (!map) return;
    setBusy(true);
    try {
      setEditKey(null);
      const saved = await siteMaps.saveFeatures(map.id, features);
      if (view.current)
        await siteMaps.update(map.id, {
          center_lng: view.current[0],
          center_lat: view.current[1],
          zoom: view.current[2],
        });
      apply(saved);
      setSelectedKey(null);
      toast.success("Map saved");
    } catch (e) {
      toast.error((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  const meta = async (body: Record<string, unknown>, ok?: string) => {
    if (!map) return;
    try {
      apply(await siteMaps.update(map.id, body), true);
      if (ok) toast.success(ok);
    } catch (e) {
      toast.error((e as Error).message);
    }
  };

  const exportGeo = async () => {
    if (!map) return;
    const gj = await siteMaps.exportGeoJson(map.id);
    const blob = new Blob([JSON.stringify(gj, null, 2)], {
      type: "application/geo+json",
    });
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = `${map.name.replace(/\W+/g, "-").toLowerCase()}.geojson`;
    a.click();
    URL.revokeObjectURL(a.href);
  };

  const importFile = async (file: File) => {
    if (!map) return;
    try {
      const text = await file.text();
      const replace = features.length
        ? window.confirm(
            "Replace the current drawing with this file? Cancel adds to it."
          )
        : false;
      apply(await siteMaps.importGeoJson(map.id, JSON.parse(text), replace));
      toast.success("Imported");
    } catch (e) {
      toast.error((e as Error).message);
    }
  };

  const uploadPlan = async (file: File) => {
    if (!map) return;
    try {
      const doc = await api.uploadDocument(
        {
          owner_type: "site_map",
          owner_id: map.id,
          filename: file.name,
          mime_type: file.type || "image/png",
          category: "plan",
        },
        file
      );
      const dims = await new Promise<[number, number]>((res, rej) => {
        const img = new Image();
        img.onload = () => res([img.naturalWidth, img.naturalHeight]);
        img.onerror = () => rej(new Error("That file is not an image"));
        img.src = URL.createObjectURL(file);
      });
      const c: [number, number] = view.current
        ? [view.current[0], view.current[1]]
        : [map.center_lng ?? 0, map.center_lat ?? 0];
      const corners = planCorners(c, dims[0], dims[1], 200, 0);
      const m = await siteMaps.update(map.id, {
        plan_document_id: doc.id,
        plan_corners: corners,
        base_layer: "plan",
      });
      apply(m, true);
      setPlanUrl((await siteMaps.plan(map.id)).url);
      toast.success("Plan placed. Adjust its size below.");
    } catch (e) {
      toast.error((e as Error).message);
    }
  };

  if (error) return <p className="text-sm text-bad">{error}</p>;
  if (!map) return <p className="text-sm text-ink-3">Loading…</p>;

  const kinds = map.kind === "apartment" ? APARTMENT_KINDS : CAMP_KINDS;
  const siteCount = features.filter((f) => f.kind === "site").length;
  const unitCount = features.filter((f) => f.kind === "unit").length;

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-3">
        <div className="min-w-0">
          <Link href="/console/maps" className="text-xs text-ink-3">
            ← Site maps
          </Link>
          <h1 className="font-display text-2xl font-bold">{map.name}</h1>
          <p className="text-sm text-ink-3">{map.property_name}</p>
        </div>
        <div className="ml-auto flex flex-wrap items-center gap-2">
          {map.published && <Badge tone="good">Published</Badge>}
          <Button variant="outline" onClick={exportGeo}>
            Export
          </Button>
          {write && (
            <>
              <label className="cursor-pointer rounded-xl border border-line bg-surface px-3 py-2 text-sm font-semibold text-ink hover:border-accent">
                Import
                <input
                  type="file"
                  accept=".geojson,.json,application/geo+json,application/json"
                  className="hidden"
                  onChange={(e) => {
                    const f = e.target.files?.[0];
                    if (f) void importFile(f);
                    e.target.value = "";
                  }}
                />
              </label>
              <Button
                variant="outline"
                onClick={() =>
                  meta(
                    { published: !map.published },
                    map.published ? "Unpublished" : "Published on your site"
                  )
                }
              >
                {map.published ? "Unpublish" : "Publish"}
              </Button>
              <Button onClick={save} disabled={!dirty || busy}>
                {dirty ? "Save map" : "Saved"}
              </Button>
            </>
          )}
        </div>
      </div>

      <div className="grid gap-3 sm:grid-cols-4">
        <StatTile
          label={map.kind === "apartment" ? "Units drawn" : "Sites drawn"}
          value={String(map.kind === "apartment" ? unitCount : siteCount)}
        />
        <StatTile
          label="Available"
          value={String(
            map.kind === "apartment"
              ? map.stats.units_available
              : map.stats.sites_available
          )}
        />
        <StatTile label="Features" value={String(features.length)} />
        <Card className="p-3 text-xs">
          <label className="font-semibold text-ink-3">
            Base layer
            <select
              className={`${field} mt-1`}
              value={map.base_layer}
              disabled={!write}
              onChange={(e) => meta({ base_layer: e.target.value })}
            >
              <option value="satellite">Satellite</option>
              <option value="streets">Streets</option>
              <option value="plan">My plan image</option>
              <option value="grid">Blank</option>
            </select>
          </label>
        </Card>
      </div>

      {write && (
        <Card className="flex flex-wrap items-center gap-2 p-3">
          <Button
            variant={tool.mode === "select" ? "primary" : "outline"}
            onClick={() => {
              setTool({ mode: "select" });
              setEditKey(null);
            }}
          >
            Select
          </Button>
          <span className="text-xs text-ink-3">Draw:</span>
          {kinds.map((k) => (
            <button
              key={k}
              onClick={() => {
                setDrawKind(k);
                setEditKey(null);
                setTool({ mode: "draw", shape: DRAW_SHAPE[k] });
              }}
              className={`rounded-full border px-3 py-1 text-xs font-bold ${
                tool.mode === "draw" && drawKind === k
                  ? "border-accent bg-accent-soft text-accent-2"
                  : "border-line bg-surface text-ink-2"
              }`}
            >
              <span
                className="mr-1.5 inline-block h-2 w-2 rounded-full"
                style={{ background: KIND_COLORS[k] }}
              />
              {KIND_LABELS[k]}
            </button>
          ))}
          {tool.mode === "draw" && (
            <span className="text-xs text-ink-3">
              {DRAW_SHAPE[drawKind] === "point"
                ? "Click to place. Press Select when done."
                : DRAW_SHAPE[drawKind] === "line"
                  ? "Click each point, double-click to finish."
                  : "Click each corner, click the first again to close."}
            </span>
          )}
        </Card>
      )}

      <div className="grid gap-4 lg:grid-cols-[1fr_20rem]">
        <div className="space-y-3">
          <div className="h-[560px] overflow-hidden rounded-2xl border border-line">
            <SiteMapCanvas
              key={map.base_layer}
              map={map}
              planUrl={planUrl}
              features={features}
              selectedKey={selectedKey}
              tool={write ? tool : { mode: "select" }}
              editKey={editKey}
              onSelect={setSelectedKey}
              onDrawn={onDrawn}
              onEdited={onEdited}
              onView={(lng, lat, z) => (view.current = [lng, lat, z])}
            />
          </div>
          <Legend kind={map.kind} />
          {map.base_layer === "plan" && write && (
            <PlanControls
              map={map}
              planUrl={planUrl}
              getView={() => view.current}
              onUpload={uploadPlan}
              onCorners={(c) => meta({ plan_corners: c })}
            />
          )}
        </div>

        <Card className="space-y-3 self-start p-4">
          {selected ? (
            <FeaturePanel
              f={selected}
              write={write}
              units={units}
              linked={linked}
              editing={editKey === selected.key}
              onPatch={(p) => patch(selected.key, p)}
              onAttr={(n, v) => setAttr(selected.key, n, v)}
              onEdit={() =>
                setEditKey(editKey === selected.key ? null : selected.key)
              }
              onDelete={() => {
                setEditKey(null);
                setFeatures((fs) => fs.filter((x) => x.key !== selected.key));
                setSelectedKey(null);
                setDirty(true);
              }}
              onRow={(count, step) => {
                const copies = rowOf(selected, count, step, (i) =>
                  nextName(selected.name, i)
                );
                setFeatures((fs) => [...fs, ...copies]);
                setDirty(true);
              }}
            />
          ) : (
            <p className="text-sm text-ink-3">
              {write
                ? "Pick a tool and draw, or select a shape to edit it."
                : "Select a shape to see its details."}
            </p>
          )}
        </Card>
      </div>
    </div>
  );
}

function Legend({ kind }: { kind: SiteMap["kind"] }) {
  const items: [string, string][] =
    kind === "apartment"
      ? [
          ["Available", UNIT_STATUS_COLORS.vacant],
          ["Occupied", UNIT_STATUS_COLORS.occupied],
          ["Notice", UNIT_STATUS_COLORS.notice],
          ["Make-ready", UNIT_STATUS_COLORS.make_ready],
          ["Not linked", KIND_COLORS.unit],
        ]
      : [
          ["Available", SITE_STATUS_COLORS.available],
          ["Booked", SITE_STATUS_COLORS.booked],
          ["Blocked", SITE_STATUS_COLORS.blocked],
          ["Out of service", SITE_STATUS_COLORS.out_of_service],
        ];
  return (
    <div className="flex flex-wrap gap-3 text-xs text-ink-2">
      {items.map(([l, c]) => (
        <span key={l} className="flex items-center gap-1.5">
          <span
            className="inline-block h-3 w-3 rounded-sm"
            style={{ background: c }}
          />
          {l}
        </span>
      ))}
    </div>
  );
}

function PlanControls({
  map,
  planUrl,
  getView,
  onUpload,
  onCorners,
}: {
  map: SiteMap;
  planUrl: string | null;
  getView: () => [number, number, number] | null;
  onUpload: (f: File) => void;
  onCorners: (c: [number, number][]) => void;
}) {
  const [width, setWidth] = useState(200);
  const [rot, setRot] = useState(0);
  const apply = async () => {
    if (!planUrl) return;
    const dims = await new Promise<[number, number]>((res) => {
      const img = new Image();
      img.onload = () => res([img.naturalWidth, img.naturalHeight]);
      img.src = planUrl;
    });
    const v = getView();
    const c: [number, number] = v
      ? [v[0], v[1]]
      : [map.center_lng ?? 0, map.center_lat ?? 0];
    onCorners(planCorners(c, dims[0], dims[1], width, rot));
  };
  return (
    <Card className="flex flex-wrap items-end gap-3 p-3 text-xs">
      <label className="cursor-pointer rounded-xl border border-line bg-surface px-3 py-2 text-sm font-semibold text-ink hover:border-accent">
        {map.plan_document_id ? "Replace plan image" : "Upload plan image"}
        <input
          type="file"
          accept="image/png,image/jpeg,image/webp"
          className="hidden"
          onChange={(e) => {
            const f = e.target.files?.[0];
            if (f) onUpload(f);
            e.target.value = "";
          }}
        />
      </label>
      {map.plan_document_id && (
        <>
          <label className="font-semibold text-ink-3">
            Width (m)
            <input
              type="number"
              className={`${field} mt-1 w-24`}
              value={width}
              min={10}
              onChange={(e) => setWidth(Number(e.target.value))}
            />
          </label>
          <label className="font-semibold text-ink-3">
            Rotation (deg)
            <input
              type="number"
              className={`${field} mt-1 w-24`}
              value={rot}
              onChange={(e) => setRot(Number(e.target.value))}
            />
          </label>
          <Button variant="outline" onClick={apply}>
            Place at map centre
          </Button>
        </>
      )}
    </Card>
  );
}

function FeaturePanel({
  f,
  write,
  units,
  linked,
  editing,
  onPatch,
  onAttr,
  onEdit,
  onDelete,
  onRow,
}: {
  f: Feature;
  write: boolean;
  units: Unit[];
  linked: Set<string | null>;
  editing: boolean;
  onPatch: (p: Partial<Feature>) => void;
  onAttr: (name: string, v: unknown) => void;
  onEdit: () => void;
  onDelete: () => void;
  onRow: (count: number, stepM: number) => void;
}) {
  const [count, setCount] = useState(5);
  const [step, setStep] = useState(8);
  const a = f.attrs;
  const num = (k: string) => (a[k] === undefined ? "" : String(a[k]));
  const setNum = (k: string, v: string) =>
    onAttr(k, v === "" ? undefined : Number(v));
  const money = (k: string) =>
    typeof a[k] === "number" ? String((a[k] as number) / 100) : "";
  const setMoney = (k: string, v: string) =>
    onAttr(k, v === "" ? undefined : Math.round(Number(v) * 100));
  const check = (k: string, label: string) => (
    <label className="flex items-center gap-1.5 text-xs">
      <input
        type="checkbox"
        disabled={!write}
        checked={a[k] === true}
        onChange={(e) => onAttr(k, e.target.checked)}
      />
      {label}
    </label>
  );

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2">
        <span
          className="inline-block h-3 w-3 rounded-sm"
          style={{ background: featureColor(f) }}
        />
        <span className="font-semibold">{KIND_LABELS[f.kind]}</span>
        {f.unit && <Badge>{f.unit.status}</Badge>}
      </div>
      <label className="block text-xs font-semibold text-ink-3">
        Name
        <input
          className={`${field} mt-1`}
          value={f.name ?? ""}
          disabled={!write}
          onChange={(e) => onPatch({ name: e.target.value || null })}
        />
      </label>
      {(f.area_m2 || f.length_m) && (
        <p className="text-xs text-ink-3">
          {f.area_m2
            ? `${Math.round(f.area_m2 * M2_TO_FT2).toLocaleString()} sq ft`
            : `${Math.round((f.length_m ?? 0) * M_TO_FT).toLocaleString()} ft`}
        </p>
      )}

      {f.kind === "unit" && (
        <label className="block text-xs font-semibold text-ink-3">
          Unit record
          <select
            className={`${field} mt-1`}
            value={f.unit_id ?? ""}
            disabled={!write}
            onChange={(e) => {
              const u = units.find((x) => x.id === e.target.value);
              onPatch({
                unit_id: e.target.value || null,
                name: u ? u.unit_number : f.name,
                unit: u
                  ? {
                      unit_number: u.unit_number,
                      status: u.status,
                      market_rent_label: u.market_rent_label,
                      beds: u.beds,
                      baths: u.baths,
                    }
                  : null,
              });
            }}
          >
            <option value="">Not linked</option>
            {units
              .filter((u) => u.id === f.unit_id || !linked.has(u.id))
              .map((u) => (
                <option key={u.id} value={u.id}>
                  Unit {u.unit_number} ({u.status})
                </option>
              ))}
          </select>
        </label>
      )}
      {f.unit && (
        <p className="text-xs text-ink-2">
          {[
            f.unit.beds ? `${f.unit.beds} bd` : null,
            f.unit.baths ? `${f.unit.baths} ba` : null,
            f.unit.market_rent_label,
          ]
            .filter(Boolean)
            .join(" · ")}
        </p>
      )}

      {f.kind === "site" && (
        <div className="space-y-2 border-t border-line pt-3">
          <div className="grid grid-cols-2 gap-2">
            <label className="text-xs font-semibold text-ink-3">
              Type
              <select
                className={`${field} mt-1`}
                disabled={!write}
                value={String(a.site_type ?? "tent")}
                onChange={(e) => onAttr("site_type", e.target.value)}
              >
                {SITE_TYPES.map((t) => (
                  <option key={t}>{t}</option>
                ))}
              </select>
            </label>
            <label className="text-xs font-semibold text-ink-3">
              Status
              <select
                className={`${field} mt-1`}
                disabled={!write}
                value={String(a.site_status ?? "available")}
                onChange={(e) => onAttr("site_status", e.target.value)}
              >
                {["available", "booked", "blocked", "out_of_service"].map(
                  (t) => (
                    <option key={t}>{t}</option>
                  )
                )}
              </select>
            </label>
            <label className="text-xs font-semibold text-ink-3">
              Max length (ft)
              <input
                className={`${field} mt-1`}
                type="number"
                disabled={!write}
                value={num("max_length_ft")}
                onChange={(e) => setNum("max_length_ft", e.target.value)}
              />
            </label>
            <label className="text-xs font-semibold text-ink-3">
              Max guests
              <input
                className={`${field} mt-1`}
                type="number"
                disabled={!write}
                value={num("max_guests")}
                onChange={(e) => setNum("max_guests", e.target.value)}
              />
            </label>
            <label className="text-xs font-semibold text-ink-3">
              Power
              <select
                className={`${field} mt-1`}
                disabled={!write}
                value={num("power_amps")}
                onChange={(e) => setNum("power_amps", e.target.value)}
              >
                <option value="">None</option>
                <option value="20">20 amp</option>
                <option value="30">30 amp</option>
                <option value="50">50 amp</option>
              </select>
            </label>
          </div>
          <div className="grid grid-cols-2 gap-1">
            {check("pull_through", "Pull-through")}
            {check("water", "Water")}
            {check("sewer", "Sewer")}
            {check("shade", "Shade")}
            {check("ada", "ADA")}
            {check("pets", "Pets OK")}
          </div>
          <div className="grid grid-cols-3 gap-2">
            {(
              [
                ["rate_cents_night", "Night $"],
                ["rate_cents_week", "Week $"],
                ["rate_cents_month", "Month $"],
              ] as const
            ).map(([k, l]) => (
              <label key={k} className="text-xs font-semibold text-ink-3">
                {l}
                <input
                  className={`${field} mt-1`}
                  type="number"
                  min={0}
                  disabled={!write}
                  value={money(k)}
                  onChange={(e) => setMoney(k, e.target.value)}
                />
              </label>
            ))}
          </div>
        </div>
      )}

      {f.kind === "amenity" && (
        <label className="block text-xs font-semibold text-ink-3">
          Amenity
          <select
            className={`${field} mt-1`}
            disabled={!write}
            value={String(a.amenity ?? "other")}
            onChange={(e) => onAttr("amenity", e.target.value)}
          >
            {AMENITIES.map((t) => (
              <option key={t} value={t}>
                {t.replace(/_/g, " ")}
              </option>
            ))}
          </select>
        </label>
      )}

      {write && (
        <div className="space-y-2 border-t border-line pt-3">
          <div className="flex gap-2">
            <Button variant="outline" onClick={onEdit}>
              {editing ? "Done editing" : "Edit shape"}
            </Button>
            <Button variant="ghost" onClick={onDelete}>
              Delete
            </Button>
          </div>
          <div className="flex items-end gap-2 text-xs">
            <label className="font-semibold text-ink-3">
              Copies
              <input
                className={`${field} mt-1 w-16`}
                type="number"
                min={1}
                max={100}
                value={count}
                onChange={(e) => setCount(Number(e.target.value))}
              />
            </label>
            <label className="font-semibold text-ink-3">
              Every (m)
              <input
                className={`${field} mt-1 w-16`}
                type="number"
                min={1}
                value={step}
                onChange={(e) => setStep(Number(e.target.value))}
              />
            </label>
            <Button
              variant="outline"
              onClick={() => onRow(Math.max(1, Math.min(100, count)), step)}
            >
              Add a row
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}
