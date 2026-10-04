"use client";

// The site map editor: draw buildings and units or campsites, link units to
// their records so they colour by status, set campsite details, place a plan
// image, import and export GeoJSON, and publish the map to your site.

import { useEffect, useMemo, useRef, useState } from "react";
import Link from "next/link";
import { useParams } from "next/navigation";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowLeft,
  Download,
  Eye,
  EyeOff,
  Map as MapIcon,
  MousePointer2,
  Save,
  Upload,
} from "lucide-react";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import {
  DRAW_SHAPE,
  KIND_COLORS,
  KIND_LABELS,
  SITE_STATUS_COLORS,
  UNIT_STATUS_COLORS,
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
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Stat } from "@/components/ui/data-table";
import { fieldClass } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";
import { BASE_LABEL, MAP_KIND_LABEL } from "../labels";
import { FeaturePanel } from "./FeaturePanel";
import { imageSize } from "./geo";
import { PlanControls } from "./PlanControls";
import { SiteMapCanvas, type Tool } from "./SiteMapCanvas";

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

export default function MapEditorPage() {
  const { id } = useParams<{ id: string }>();
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const map = useQuery({
    queryKey: ["site-maps", id],
    queryFn: () => siteMaps.get(id),
    enabled: scoped && can("property:read"),
    refetchOnWindowFocus: false,
  });

  const back = (
    <Link
      href="/console/maps"
      className="inline-flex items-center gap-1.5 text-[13px] text-fg-3 transition hover:text-fg"
    >
      <ArrowLeft className="size-4" />
      Site maps
    </Link>
  );

  if (map.error)
    return (
      <div className="space-y-6">
        {back}
        <Panel className="mx-auto max-w-lg">
          <EmptyState
            icon={<MapIcon />}
            title="Couldn't load this map"
            description={map.error.message}
          />
        </Panel>
      </div>
    );
  if (!map.data)
    return (
      <div className="space-y-6">
        {back}
        <Skeleton className="h-20 rounded-2xl" />
        <Skeleton className="h-[560px] rounded-2xl" />
      </div>
    );
  return <Editor key={map.data.id} initial={map.data} back={back} />;
}

function Editor({
  initial,
  back,
}: {
  initial: SiteMap;
  back: React.ReactNode;
}) {
  const { can } = useAuth();
  const write = can("property:write");
  const qc = useQueryClient();
  const [map, setMap] = useState(initial);
  const [features, setFeatures] = useState<Feature[]>(() =>
    fromServer(initial)
  );
  const [dirty, setDirty] = useState(false);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [tool, setTool] = useState<Tool>({ mode: "select" });
  const [drawKind, setDrawKind] = useState<FeatureKind>(
    initial.kind === "apartment" ? "unit" : "site"
  );
  const [editKey, setEditKey] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [pending, setPending] = useState<unknown>(null);
  const view = useRef<[number, number, number] | null>(null);
  const importInput = useRef<HTMLInputElement>(null);

  const units = useQuery({
    queryKey: ["properties", map.property_id, "units"],
    queryFn: () => api.units(map.property_id),
    enabled: can("lease:read") || can("property:read"),
  });
  const plan = useQuery({
    queryKey: ["site-map-plan", map.id, map.plan_document_id],
    queryFn: () => siteMaps.plan(map.id),
    enabled: !!map.plan_document_id,
    staleTime: 10 * 60_000,
  });
  const planUrl = plan.data?.url ?? null;

  /** Take the server's copy; the drawing too unless asked to keep it. */
  const apply = (m: SiteMap, keepDrawing = false) => {
    setMap(m);
    qc.setQueryData(["site-maps", m.id], m);
    void qc.invalidateQueries({ queryKey: ["site-maps"], exact: true });
    if (!keepDrawing) {
      setFeatures(fromServer(m));
      setDirty(false);
    }
  };

  // Leaving with an unsaved drawing asks first.
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
    // Points (campsites, amenities) keep placing; shapes go back to select.
    if (DRAW_SHAPE[kind] !== "point") setTool({ mode: "select" });
  };

  async function save() {
    setBusy(true);
    try {
      setEditKey(null);
      const saved = await siteMaps.saveFeatures(map.id, features);
      let m = saved;
      if (view.current)
        m = await siteMaps.update(map.id, {
          center_lng: view.current[0],
          center_lat: view.current[1],
          zoom: view.current[2],
        });
      apply({ ...m, features: saved.features });
      setSelectedKey(null);
      toast.success("Map saved");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't save the map");
    } finally {
      setBusy(false);
    }
  }

  async function meta(body: Record<string, unknown>, ok?: string) {
    try {
      apply(await siteMaps.update(map.id, body), true);
      if (ok) toast.success(ok);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't change the map");
    }
  }

  async function exportGeo() {
    try {
      const gj = await siteMaps.exportGeoJson(map.id);
      const blob = new Blob([JSON.stringify(gj, null, 2)], {
        type: "application/geo+json",
      });
      const a = document.createElement("a");
      a.href = URL.createObjectURL(blob);
      a.download = `${map.name.replace(/\W+/g, "-").toLowerCase()}.geojson`;
      document.body.appendChild(a);
      a.click();
      a.remove();
      URL.revokeObjectURL(a.href);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't export it");
    }
  }

  async function runImport(geojson: unknown, replace: boolean) {
    setPending(null);
    try {
      apply(await siteMaps.importGeoJson(map.id, geojson, replace));
      setSelectedKey(null);
      setEditKey(null);
      toast.success("Imported");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't import it");
    }
  }

  async function pickImport(file: File) {
    let geojson: unknown;
    try {
      geojson = JSON.parse(await file.text());
    } catch {
      toast.error("That file isn't GeoJSON");
      return;
    }
    if (features.length) setPending(geojson);
    else void runImport(geojson, false);
  }

  async function uploadPlan(file: File) {
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
      const dims = await imageSize(URL.createObjectURL(file));
      const c: [number, number] = view.current
        ? [view.current[0], view.current[1]]
        : [map.center_lng ?? 0, map.center_lat ?? 0];
      const m = await siteMaps.update(map.id, {
        plan_document_id: doc.id,
        plan_corners: planCorners(c, dims[0], dims[1], 200, 0),
        base_layer: "plan",
      });
      apply(m, true);
      toast.success("Plan placed. Adjust its size below.");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't place the plan");
    }
  }

  const apt = map.kind === "apartment";
  const kinds = apt ? APARTMENT_KINDS : CAMP_KINDS;
  const drawn = features.filter((f) => f.kind === (apt ? "unit" : "site"));

  return (
    <div className="space-y-6">
      {back}
      <PageHeader
        eyebrow={
          <span className="inline-flex items-center gap-2">
            {MAP_KIND_LABEL[map.kind] ?? map.kind}
            {map.published && <Badge tone="good">published</Badge>}
            {dirty && <Badge tone="warn">unsaved</Badge>}
          </span>
        }
        title={map.name}
        description={map.property_name}
        actions={
          <div className="flex flex-wrap items-center justify-end gap-2">
            <Button variant="secondary" size="sm" onClick={exportGeo}>
              <Download />
              Export
            </Button>
            {write && (
              <>
                <input
                  ref={importInput}
                  type="file"
                  accept=".geojson,.json,application/geo+json,application/json"
                  className="hidden"
                  onChange={(e) => {
                    const f = e.target.files?.[0];
                    if (f) void pickImport(f);
                    e.target.value = "";
                  }}
                />
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => importInput.current?.click()}
                >
                  <Upload />
                  Import
                </Button>
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() =>
                    meta(
                      { published: !map.published },
                      map.published ? "Unpublished" : "Published on your site"
                    )
                  }
                >
                  {map.published ? <EyeOff /> : <Eye />}
                  {map.published ? "Unpublish" : "Publish"}
                </Button>
                <Button
                  size="sm"
                  onClick={save}
                  disabled={!dirty}
                  loading={busy}
                >
                  <Save />
                  {dirty ? "Save map" : "Saved"}
                </Button>
              </>
            )}
          </div>
        }
      />

      <section className="grid grid-cols-2 gap-3 xl:grid-cols-4">
        <Stat
          label={apt ? "Units drawn" : "Sites drawn"}
          value={drawn.length}
        />
        <Stat
          label="Available"
          value={apt ? map.stats.units_available : map.stats.sites_available}
          tone="good"
        />
        <Stat label="Shapes" value={features.length} />
        <div className="glass rounded-2xl p-4">
          <label className="eyebrow block" htmlFor="base-layer">
            Base layer
          </label>
          <select
            id="base-layer"
            className={`${fieldClass} mt-1.5 w-full`}
            value={map.base_layer}
            disabled={!write}
            onChange={(e) => meta({ base_layer: e.target.value })}
          >
            {Object.entries(BASE_LABEL).map(([k, l]) => (
              <option key={k} value={k}>
                {l}
              </option>
            ))}
          </select>
        </div>
      </section>

      {write && (
        <Panel className="flex flex-wrap items-center gap-2 p-2.5">
          <Button
            size="sm"
            variant={tool.mode === "select" ? "primary" : "secondary"}
            onClick={() => {
              setTool({ mode: "select" });
              setEditKey(null);
            }}
          >
            <MousePointer2 />
            Select
          </Button>
          <span className="px-1 text-xs text-fg-3">Draw</span>
          {kinds.map((k) => {
            const on = tool.mode === "draw" && drawKind === k;
            return (
              <button
                key={k}
                type="button"
                aria-pressed={on}
                onClick={() => {
                  setDrawKind(k);
                  setEditKey(null);
                  setTool({ mode: "draw", shape: DRAW_SHAPE[k] });
                }}
                className={cn(
                  "inline-flex items-center gap-1.5 rounded-lg border px-2.5 py-1 text-xs font-medium transition",
                  on
                    ? "border-accent bg-accent/12 text-accent"
                    : "border-line text-fg-2 hover:bg-fill-2 hover:text-fg"
                )}
              >
                <span
                  className="size-2 rounded-full"
                  style={{ background: KIND_COLORS[k] }}
                />
                {KIND_LABELS[k]}
              </button>
            );
          })}
          <span className="basis-full text-xs text-fg-3 sm:ml-auto sm:basis-auto">
            {tool.mode === "draw"
              ? DRAW_SHAPE[drawKind] === "point"
                ? "Click to place. Press Select when done."
                : DRAW_SHAPE[drawKind] === "line"
                  ? "Click each point. Double-click or press Enter to finish."
                  : "Click each corner. Click the first one again, or press Enter, to close."
              : editKey
                ? "Drag corners or the shape. Drag a midpoint to add a corner; Alt-click or right-click one to remove it."
                : "Drag to move around, scroll to zoom, click a shape to select it."}
          </span>
        </Panel>
      )}

      <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_20rem]">
        <div className="space-y-3">
          <div className="h-[560px] overflow-hidden rounded-2xl border border-line">
            <SiteMapCanvas
              map={map}
              planUrl={planUrl}
              features={features}
              selectedKey={selectedKey}
              tool={write ? tool : { mode: "select" }}
              editKey={editKey}
              onSelect={setSelectedKey}
              onDrawn={onDrawn}
              onEdited={(key, geometry) => patch(key, { geometry })}
              onView={(lng, lat, z) => (view.current = [lng, lat, z])}
            />
          </div>
          <Legend kind={map.kind} />
          {map.base_layer === "plan" && write && (
            <PlanControls
              hasPlan={!!map.plan_document_id}
              planUrl={planUrl}
              center={() =>
                view.current
                  ? [view.current[0], view.current[1]]
                  : [map.center_lng ?? 0, map.center_lat ?? 0]
              }
              onUpload={uploadPlan}
              onCorners={(c) => meta({ plan_corners: c }, "Plan placed")}
            />
          )}
        </div>

        <Panel className="self-start p-4">
          {selected ? (
            <FeaturePanel
              key={selected.key}
              f={selected}
              write={write}
              units={units.data ?? []}
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
              onClose={() => {
                setEditKey(null);
                setSelectedKey(null);
              }}
            />
          ) : (
            <EmptyState
              icon={<MousePointer2 />}
              title="Nothing selected"
              description={
                write
                  ? "Pick a tool and draw, or click a shape to edit it."
                  : "Click a shape to see its details."
              }
              className="py-8"
            />
          )}
        </Panel>
      </div>

      <Dialog
        open={pending !== null}
        onOpenChange={(o) => !o && setPending(null)}
      >
        <DialogContent>
          <DialogTitle className="text-[17px] font-semibold">
            Import this file
          </DialogTitle>
          <DialogDescription className="mt-1 text-[13px] text-fg-3">
            Replace the current drawing with it, or add its shapes to
            what&apos;s there.
            {dirty ? " Unsaved changes are lost either way." : ""}
          </DialogDescription>
          <div className="mt-5 flex justify-end gap-2">
            <Button variant="ghost" onClick={() => setPending(null)}>
              Cancel
            </Button>
            <Button
              variant="secondary"
              onClick={() => runImport(pending, false)}
            >
              Add to it
            </Button>
            <Button variant="danger" onClick={() => runImport(pending, true)}>
              Replace
            </Button>
          </div>
        </DialogContent>
      </Dialog>
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
          ["Down", UNIT_STATUS_COLORS.down],
          ["Not linked", KIND_COLORS.unit],
        ]
      : [
          ["Available", SITE_STATUS_COLORS.available],
          ["Booked", SITE_STATUS_COLORS.booked],
          ["Blocked", SITE_STATUS_COLORS.blocked],
          ["Out of service", SITE_STATUS_COLORS.out_of_service],
        ];
  return (
    <div className="flex flex-wrap gap-x-4 gap-y-1.5 px-1 text-xs text-fg-2">
      {items.map(([l, c]) => (
        <span key={l} className="inline-flex items-center gap-1.5">
          <span className="size-3 rounded-sm" style={{ background: c }} />
          {l}
        </span>
      ))}
    </div>
  );
}
