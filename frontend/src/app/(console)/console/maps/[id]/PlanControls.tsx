"use client";

// Upload a plan image (a site plan, a campground brochure map) and size and
// turn it over the map centre.

import { useRef, useState } from "react";
import { Crosshair, ImageUp } from "lucide-react";
import { toast } from "sonner";
import { planCorners } from "@/lib/sitemaps";
import { Button } from "@/components/ui/button";
import { fieldClass } from "@/components/ui/input";
import { Panel } from "@/components/ui/panel";
import { F } from "@/components/property/bits";
import { imageSize } from "./geo";

export function PlanControls({
  hasPlan,
  planUrl,
  center,
  onUpload,
  onCorners,
}: {
  hasPlan: boolean;
  planUrl: string | null;
  center: () => [number, number];
  onUpload: (f: File) => Promise<void>;
  onCorners: (c: [number, number][]) => void;
}) {
  const [width, setWidth] = useState(200);
  const [rot, setRot] = useState(0);
  const [busy, setBusy] = useState(false);
  const file = useRef<HTMLInputElement>(null);

  async function place() {
    if (!planUrl) return;
    try {
      const [w, h] = await imageSize(planUrl);
      onCorners(planCorners(center(), w, h, width, rot));
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't read the plan");
    }
  }

  return (
    <Panel className="flex flex-wrap items-end gap-3 p-3">
      <input
        ref={file}
        type="file"
        accept="image/png,image/jpeg,image/webp"
        className="hidden"
        onChange={async (e) => {
          const f = e.target.files?.[0];
          e.target.value = "";
          if (!f) return;
          setBusy(true);
          await onUpload(f);
          setBusy(false);
        }}
      />
      <Button
        size="sm"
        variant="secondary"
        loading={busy}
        onClick={() => file.current?.click()}
      >
        <ImageUp />
        {hasPlan ? "Replace plan image" : "Upload plan image"}
      </Button>
      {hasPlan && (
        <>
          <F label="Width (m)">
            <input
              type="number"
              min={10}
              className={`${fieldClass} w-24`}
              value={width}
              onChange={(e) => setWidth(Number(e.target.value))}
            />
          </F>
          <F label="Rotation (degrees)">
            <input
              type="number"
              className={`${fieldClass} w-24`}
              value={rot}
              onChange={(e) => setRot(Number(e.target.value))}
            />
          </F>
          <Button
            size="sm"
            variant="secondary"
            disabled={!planUrl || width < 1}
            onClick={place}
          >
            <Crosshair />
            Place at map centre
          </Button>
        </>
      )}
      {!hasPlan && (
        <p className="text-xs text-fg-3">
          A PNG, JPEG or WebP. It lands 200 m wide at the map centre.
        </p>
      )}
    </Panel>
  );
}
