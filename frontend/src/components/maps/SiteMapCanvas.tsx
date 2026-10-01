"use client";

// The site map canvas: MapLibre for the base layer and the drawn features,
// Terra Draw for drawing new shapes and editing one shape's geometry. Loaded
// in the browser only (MapLibre needs a window).

import "maplibre-gl/dist/maplibre-gl.css";
import * as maplibregl from "maplibre-gl";

// The worker files are copied into public/ at dev and build time (see
// scripts/copy-maplibre-worker.mjs); the bundle can't find them on its own.
if (typeof window !== "undefined") {
  maplibregl.setWorkerUrl("/maplibre/maplibre-gl-worker.mjs");
}
import { useEffect, useRef, useState } from "react";
import {
  TerraDraw,
  TerraDrawLineStringMode,
  TerraDrawPointMode,
  TerraDrawPolygonMode,
  TerraDrawSelectMode,
} from "terra-draw";
import { TerraDrawMapLibreGLAdapter } from "terra-draw-maplibre-gl-adapter";
import {
  featureColor,
  type Feature,
  type Geometry,
  type SiteMap,
} from "@/lib/sitemaps";

const STREETS = "https://tiles.openfreemap.org/styles/liberty";
const SAT_TILES =
  "https://basemap.nationalmap.gov/arcgis/rest/services/USGSImageryOnly/MapServer/tile/{z}/{y}/{x}";
const EMPTY = { type: "FeatureCollection" as const, features: [] };

export type Tool =
  | { mode: "select" }
  | { mode: "draw"; shape: "point" | "polygon" | "line" };

interface Props {
  map: Pick<
    SiteMap,
    "base_layer" | "center_lng" | "center_lat" | "zoom" | "plan_corners"
  >;
  planUrl: string | null;
  features: Feature[];
  selectedKey: string | null;
  tool: Tool;
  /** The feature whose shape is being edited, if any. */
  editKey: string | null;
  onSelect: (key: string | null) => void;
  onDrawn: (geometry: Geometry) => void;
  onEdited: (key: string, geometry: Geometry) => void;
  onView?: (lng: number, lat: number, zoom: number) => void;
}

function style(
  layer: SiteMap["base_layer"]
): maplibregl.StyleSpecification | string {
  if (layer === "streets") return STREETS;
  if (layer === "satellite") {
    return {
      version: 8,
      sources: {
        sat: {
          type: "raster",
          tiles: [SAT_TILES],
          tileSize: 256,
          maxzoom: 19,
          attribution: "USGS The National Map",
        },
      },
      layers: [{ id: "sat", type: "raster", source: "sat" }],
    };
  }
  // plan and grid: no basemap, a pale ground for the plan or grid to sit on.
  return {
    version: 8,
    sources: {},
    layers: [
      {
        id: "bg",
        type: "background",
        paint: { "background-color": "#f4f1ea" },
      },
    ],
  };
}

function toGeoJson(
  features: Feature[],
  selectedKey: string | null,
  hide: string | null
) {
  return {
    type: "FeatureCollection" as const,
    features: features
      .filter((f) => f.key !== hide)
      .map((f) => ({
        type: "Feature" as const,
        id: undefined,
        geometry: f.geometry as unknown as GeoJSON.Geometry,
        properties: {
          key: f.key,
          kind: f.kind,
          color: featureColor(f),
          selected: f.key === selectedKey,
          label: f.name ?? "",
        },
      })),
  };
}

export default function SiteMapCanvas(props: Props) {
  const box = useRef<HTMLDivElement>(null);
  const mapRef = useRef<maplibregl.Map | null>(null);
  const drawRef = useRef<TerraDraw | null>(null);
  const [ready, setReady] = useState(false);
  const latest = useRef(props);
  useEffect(() => {
    latest.current = props;
  });
  const editId = useRef<string | number | null>(null);

  // Create the map once; the base layer is part of the key in the parent.
  useEffect(() => {
    if (!box.current) return;
    const p = latest.current;
    let map: maplibregl.Map;
    try {
      map = new maplibregl.Map({
        container: box.current,
        style: style(p.map.base_layer),
        center: [p.map.center_lng ?? -98.5, p.map.center_lat ?? 39.5],
        zoom: p.map.center_lng === null ? 3.5 : p.map.zoom,
        maxZoom: 22,
        dragRotate: false,
        attributionControl: { compact: true },
      });
    } catch {
      return; // no WebGL
    }
    mapRef.current = map;
    map.addControl(
      new maplibregl.NavigationControl({ showCompass: false }),
      "top-right"
    );
    map.addControl(
      new maplibregl.ScaleControl({ unit: "imperial" }),
      "bottom-left"
    );
    map.on("moveend", () => {
      const c = map.getCenter();
      latest.current.onView?.(c.lng, c.lat, map.getZoom());
    });
    map.once("style.load", () => {
      map.addSource("feat", { type: "geojson", data: EMPTY });
      map.addLayer({
        id: "feat-fill",
        type: "fill",
        source: "feat",
        filter: ["==", ["geometry-type"], "Polygon"],
        paint: {
          "fill-color": ["get", "color"],
          "fill-opacity": [
            "case",
            ["get", "selected"],
            0.55,
            ["==", ["get", "kind"], "boundary"],
            0.06,
            0.32,
          ],
        },
      });
      map.addLayer({
        id: "feat-line",
        type: "line",
        source: "feat",
        filter: [
          "in",
          ["geometry-type"],
          ["literal", ["Polygon", "LineString"]],
        ],
        paint: {
          "line-color": ["get", "color"],
          "line-width": ["case", ["get", "selected"], 4, 2],
        },
      });
      map.addLayer({
        id: "feat-point",
        type: "circle",
        source: "feat",
        filter: ["==", ["geometry-type"], "Point"],
        paint: {
          "circle-color": ["get", "color"],
          "circle-radius": ["case", ["get", "selected"], 9, 6],
          "circle-stroke-color": "#ffffff",
          "circle-stroke-width": 2,
        },
      });
      map.addLayer({
        id: "feat-label",
        type: "symbol",
        source: "feat",
        layout: {
          "text-field": ["get", "label"],
          "text-size": 11,
          "text-offset": [0, 1.1],
          "text-allow-overlap": false,
          "text-font": ["Noto Sans Regular"],
        },
        paint: {
          "text-color": "#111827",
          "text-halo-color": "#ffffff",
          "text-halo-width": 1.5,
        },
      });

      const draw = new TerraDraw({
        adapter: new TerraDrawMapLibreGLAdapter({ map }),
        modes: [
          new TerraDrawSelectMode({
            flags: {
              polygon: {
                feature: {
                  draggable: true,
                  coordinates: {
                    midpoints: true,
                    draggable: true,
                    deletable: true,
                  },
                },
              },
              linestring: {
                feature: {
                  draggable: true,
                  coordinates: {
                    midpoints: true,
                    draggable: true,
                    deletable: true,
                  },
                },
              },
              point: { feature: { draggable: true } },
            },
          }),
          new TerraDrawPointMode(),
          new TerraDrawPolygonMode(),
          new TerraDrawLineStringMode(),
        ],
      });
      draw.start();
      drawRef.current = draw;
      draw.on("finish", (id, ctx) => {
        const f = draw.getSnapshotFeature(id);
        if (!f) return;
        if (ctx.mode === "select") return;
        latest.current.onDrawn(f.geometry as unknown as Geometry);
        draw.removeFeatures([id]);
      });
      draw.on("change", (ids, type) => {
        if (type === "delete" || editId.current === null) return;
        if (!ids.includes(editId.current)) return;
        const f = draw.getSnapshotFeature(editId.current);
        const k = latest.current.editKey;
        if (f && k)
          latest.current.onEdited(k, f.geometry as unknown as Geometry);
      });
      map.on("click", (e) => {
        if (latest.current.tool.mode !== "select" || latest.current.editKey)
          return;
        const hit = map.queryRenderedFeatures(e.point, {
          layers: ["feat-point", "feat-label", "feat-fill", "feat-line"],
        })[0];
        latest.current.onSelect((hit?.properties?.key as string) ?? null);
      });
      setReady(true);
    });
    return () => {
      drawRef.current?.stop();
      drawRef.current = null;
      map.remove();
      mapRef.current = null;
      setReady(false);
    };
  }, []);

  // Plan image under the features.
  useEffect(() => {
    const map = mapRef.current;
    if (!ready || !map) return;
    if (map.getLayer("plan")) map.removeLayer("plan");
    if (map.getSource("plan")) map.removeSource("plan");
    const { planUrl, map: m } = props;
    if (m.base_layer === "plan" && planUrl && m.plan_corners?.length === 4) {
      map.addSource("plan", {
        type: "image",
        url: planUrl,
        coordinates: m.plan_corners as [
          [number, number],
          [number, number],
          [number, number],
          [number, number],
        ],
      });
      map.addLayer({ id: "plan", type: "raster", source: "plan" }, "feat-fill");
    }
  }, [ready, props]);

  // Features.
  useEffect(() => {
    const map = mapRef.current;
    if (!ready || !map) return;
    (map.getSource("feat") as maplibregl.GeoJSONSource | undefined)?.setData(
      toGeoJson(props.features, props.selectedKey, props.editKey)
    );
  }, [ready, props.features, props.selectedKey, props.editKey]);

  // Tool.
  useEffect(() => {
    const draw = drawRef.current;
    if (!ready || !draw) return;
    if (props.editKey) return;
    if (props.tool.mode === "select") draw.setMode("static");
    else
      draw.setMode(
        props.tool.shape === "line"
          ? "linestring"
          : props.tool.shape === "point"
            ? "point"
            : "polygon"
      );
  }, [ready, props.tool, props.editKey]);

  // Editing one shape: move it into Terra Draw, select it, hand it back on exit.
  useEffect(() => {
    const draw = drawRef.current;
    if (!ready || !draw) return;
    const key = props.editKey;
    if (!key) {
      if (editId.current !== null) {
        draw.removeFeatures([editId.current]);
        editId.current = null;
        draw.setMode("static");
      }
      return;
    }
    const f = latest.current.features.find((x) => x.key === key);
    if (!f) return;
    const mode =
      f.geometry.type === "Point"
        ? "point"
        : f.geometry.type === "LineString"
          ? "linestring"
          : "polygon";
    const res = draw.addFeatures([
      {
        type: "Feature",
        geometry: f.geometry as unknown as GeoJSON.Geometry,
        properties: { mode },
      },
    ] as never);
    const id = res?.[0]?.id ?? null;
    if (id !== null && id !== undefined) {
      editId.current = id;
      draw.setMode("select");
      draw.selectFeature(id);
    }
  }, [ready, props.editKey]);

  // Fly to a selected feature on request is left to the parent via the view props.
  return <div ref={box} className="h-full w-full" />;
}
