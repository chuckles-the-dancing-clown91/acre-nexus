"use client";

// The photos at the top of a property: one large and four small, a full-screen
// viewer with arrow keys, floor plans on their own tab, and upload for people
// who can edit.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Camera, ChevronLeft, ChevronRight, ImagePlus, X } from "lucide-react";
import { toast } from "sonner";
import { api, type PropertyMediaItem } from "@/lib/api";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { cn } from "@/lib/utils";
import { why } from "./bits";

type Tab = "photos" | "plans";

export function Gallery({
  propertyId,
  name,
  fallbackUrl,
  manage,
}: {
  propertyId: string;
  name: string;
  /** The street photo, when no photos have been uploaded. */
  fallbackUrl: string | null;
  manage: boolean;
}) {
  const qc = useQueryClient();
  const media = useQuery({
    queryKey: ["property-media", propertyId],
    queryFn: () => api.propertyMedia(propertyId),
  });
  const file = useRef<HTMLInputElement>(null);
  const [tab, setTab] = useState<Tab>("photos");
  const [open, setOpen] = useState(false);
  const [at, setAt] = useState(0);
  const [busy, setBusy] = useState(false);

  const items = useMemo(
    () => (media.data?.items ?? []).filter((m) => m.url),
    [media.data]
  );
  const photos = useMemo(() => {
    const p = items.filter((m) => m.category !== "floorplan");
    // The cover leads.
    return [...p].sort((a, b) => Number(b.is_hero) - Number(a.is_hero));
  }, [items]);
  const plans = items.filter((m) => m.category === "floorplan");
  const shown = tab === "photos" ? photos : plans;

  const step = useCallback(
    (d: number) => setAt((i) => (i + d + shown.length) % shown.length),
    [shown.length]
  );
  useEffect(() => {
    if (!open) return;
    const on = (e: KeyboardEvent) => {
      if (e.key === "ArrowRight") step(1);
      if (e.key === "ArrowLeft") step(-1);
    };
    window.addEventListener("keydown", on);
    return () => window.removeEventListener("keydown", on);
  }, [open, step]);

  async function upload(list: FileList | null) {
    if (!list?.length) return;
    setBusy(true);
    try {
      for (const f of Array.from(list)) {
        await api.uploadDocument(
          {
            owner_type: "property",
            owner_id: propertyId,
            filename: f.name,
            mime_type: f.type || "image/jpeg",
            category: "photo",
          },
          f
        );
      }
      toast.success(list.length === 1 ? "Photo added" : "Photos added");
      void qc.invalidateQueries({ queryKey: ["property-media", propertyId] });
      void qc.invalidateQueries({ queryKey: ["properties", propertyId] });
    } catch (e) {
      toast.error(why(e, "Upload failed"));
    } finally {
      setBusy(false);
      if (file.current) file.current.value = "";
    }
  }

  const lead = photos[0]?.url ?? fallbackUrl;
  const side = photos.slice(1, 5);

  function show(list: Tab, i: number) {
    setTab(list);
    setAt(i);
    setOpen(true);
  }

  return (
    <div className="relative h-full">
      <div className="grid h-64 grid-cols-4 grid-rows-2 gap-1.5 overflow-hidden rounded-2xl sm:h-80 xl:h-full xl:min-h-[26rem]">
        <button
          type="button"
          onClick={() => lead && show("photos", 0)}
          disabled={!lead}
          className={cn(
            "relative overflow-hidden bg-fill",
            side.length ? "col-span-2 row-span-2" : "col-span-4 row-span-2"
          )}
          aria-label={`Open the photos of ${name}`}
        >
          {lead ? (
            // eslint-disable-next-line @next/next/no-img-element -- signed blob URL
            <img src={lead} alt={name} className="size-full object-cover" />
          ) : (
            <span className="flex size-full flex-col items-center justify-center gap-2 bg-[radial-gradient(120%_120%_at_0%_0%,color-mix(in_oklab,var(--accent)_28%,transparent),transparent)] text-fg-3">
              <Camera className="size-8" />
              <span className="text-[13px]">No photos yet</span>
            </span>
          )}
        </button>
        {side.map((m, i) => (
          <button
            key={m.document_id}
            type="button"
            onClick={() => show("photos", i + 1)}
            className="relative overflow-hidden bg-fill"
            aria-label={`Open photo ${i + 2}`}
          >
            {/* eslint-disable-next-line @next/next/no-img-element -- signed blob URL */}
            <img
              src={m.url!}
              alt={m.filename}
              className="size-full object-cover transition duration-300 hover:scale-105"
            />
          </button>
        ))}
      </div>
      <div className="absolute right-3 bottom-3 flex gap-2">
        {plans.length > 0 && (
          <Button
            size="sm"
            variant="secondary"
            onClick={() => show("plans", 0)}
          >
            Floor plans · {plans.length}
          </Button>
        )}
        {photos.length > 0 && (
          <Button
            size="sm"
            variant="secondary"
            onClick={() => show("photos", 0)}
          >
            See all {photos.length} photos
          </Button>
        )}
        {manage && (
          <>
            <input
              ref={file}
              type="file"
              accept="image/*"
              multiple
              className="hidden"
              onChange={(e) => upload(e.target.files)}
            />
            <Button
              size="sm"
              variant="secondary"
              disabled={busy}
              onClick={() => file.current?.click()}
            >
              <ImagePlus />
              Add photos
            </Button>
          </>
        )}
      </div>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent
          hideClose
          className="flex h-[90dvh] max-w-5xl flex-col bg-black/90 p-4 text-white"
        >
          <DialogTitle className="sr-only">{name} photos</DialogTitle>
          <DialogDescription className="sr-only">
            Use the arrow keys to move between photos.
          </DialogDescription>
          <div className="flex items-center gap-2">
            <div className="flex gap-1 rounded-xl bg-white/10 p-1">
              {(["photos", "plans"] as const).map((t) => {
                const n = t === "photos" ? photos.length : plans.length;
                if (t === "plans" && n === 0) return null;
                return (
                  <button
                    key={t}
                    type="button"
                    onClick={() => {
                      setTab(t);
                      setAt(0);
                    }}
                    className={cn(
                      "rounded-lg px-3 py-1 text-[13px] font-medium",
                      tab === t ? "bg-white/20" : "text-white/60"
                    )}
                  >
                    {t === "photos" ? "Photos" : "Floor plans"} · {n}
                  </button>
                );
              })}
            </div>
            <span className="ml-auto text-xs text-white/60">
              {shown.length ? `${at + 1} of ${shown.length}` : ""}
            </span>
            <button
              type="button"
              aria-label="Close"
              onClick={() => setOpen(false)}
              className="rounded-lg p-1.5 text-white/70 hover:bg-white/10"
            >
              <X className="size-5" />
            </button>
          </div>
          <div className="relative mt-3 min-h-0 flex-1">
            {shown[at] && (
              // eslint-disable-next-line @next/next/no-img-element -- signed blob URL
              <img
                src={shown[at].url!}
                alt={(shown[at] as PropertyMediaItem).filename}
                className="size-full object-contain"
              />
            )}
            {shown.length > 1 && (
              <>
                <button
                  type="button"
                  aria-label="Previous"
                  onClick={() => step(-1)}
                  className="absolute top-1/2 left-1 -translate-y-1/2 rounded-full bg-black/50 p-2 hover:bg-black/70"
                >
                  <ChevronLeft className="size-6" />
                </button>
                <button
                  type="button"
                  aria-label="Next"
                  onClick={() => step(1)}
                  className="absolute top-1/2 right-1 -translate-y-1/2 rounded-full bg-black/50 p-2 hover:bg-black/70"
                >
                  <ChevronRight className="size-6" />
                </button>
              </>
            )}
          </div>
          {shown.length > 1 && (
            <div className="mt-3 flex gap-1.5 overflow-x-auto pb-1">
              {shown.map((m, i) => (
                <button
                  key={m.document_id}
                  type="button"
                  onClick={() => setAt(i)}
                  aria-label={`Photo ${i + 1}`}
                  className={cn(
                    "h-14 w-20 shrink-0 overflow-hidden rounded-md border-2",
                    i === at ? "border-white" : "border-transparent opacity-60"
                  )}
                >
                  {/* eslint-disable-next-line @next/next/no-img-element -- signed blob URL */}
                  <img src={m.url!} alt="" className="size-full object-cover" />
                </button>
              ))}
            </div>
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}
