"use client";

// A listing's photos: the first is the cover. Each needs a few words saying
// what it shows, for screen readers and the portals' captions.

import { useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ImagePlus, Star, X } from "lucide-react";
import { toast } from "sonner";
import {
  altFromFilename,
  listingPhotos,
  type ListingPhoto,
} from "@/lib/syndication";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

export function ListingPhotos({
  listingId,
  title,
  onChange,
}: {
  listingId: string;
  title: string;
  onChange: () => void;
}) {
  const qc = useQueryClient();
  const key = ["listing-photos", listingId];
  const photos = useQuery({
    queryKey: key,
    queryFn: () => listingPhotos.list(listingId),
  });
  const input = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const list = photos.data ?? [];

  const done = (next: ListingPhoto[]) => {
    qc.setQueryData(key, next);
    onChange();
  };

  async function add(files: FileList) {
    setBusy(true);
    let n = list.length;
    try {
      for (const f of Array.from(files)) {
        if (!f.type.startsWith("image/")) continue;
        n += 1;
        done(
          await listingPhotos.add(
            listingId,
            f,
            altFromFilename(f.name, `${title}, photo ${n}`)
          )
        );
      }
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't add the photo");
    } finally {
      setBusy(false);
    }
  }

  async function act(p: Promise<ListingPhoto[]>) {
    try {
      done(await p);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't change it");
    }
  }

  return (
    <div>
      <div className="mb-2 flex items-center justify-between">
        <span className="text-xs text-fg-3">
          Photos{list.length ? ` · ${list.length}` : ""} · the first is the
          cover
        </span>
        <Button
          size="sm"
          variant="secondary"
          onClick={() => input.current?.click()}
          disabled={busy}
        >
          <ImagePlus />
          {busy ? "Uploading…" : "Add photos"}
        </Button>
        <input
          ref={input}
          type="file"
          accept="image/*"
          multiple
          className="hidden"
          aria-label="Add photos"
          onChange={(e) => {
            if (e.target.files?.length) void add(e.target.files);
            e.target.value = "";
          }}
        />
      </div>
      {list.length === 0 && !photos.isLoading && (
        <p className="rounded-xl border border-dashed border-line px-3 py-6 text-center text-[13px] text-fg-3">
          No photos yet. The portals won&apos;t take a listing without one.
        </p>
      )}
      <ul className="grid grid-cols-2 gap-2 sm:grid-cols-3">
        {list.map((p, i) => (
          <li
            key={p.id}
            className="overflow-hidden rounded-xl border border-line bg-fill/40"
          >
            <div className="relative aspect-[4/3] bg-fill">
              {p.preview_url && (
                // eslint-disable-next-line @next/next/no-img-element -- signed blob URL
                <img
                  src={p.preview_url}
                  alt={p.alt_text}
                  className="size-full object-cover"
                />
              )}
              {i === 0 && (
                <span className="absolute top-1.5 left-1.5 rounded-md bg-black/60 px-1.5 py-0.5 text-[11px] font-medium text-white">
                  Cover
                </span>
              )}
              <div className="absolute top-1.5 right-1.5 flex gap-1">
                {i > 0 && (
                  <button
                    type="button"
                    aria-label="Make this the cover"
                    title="Make this the cover"
                    onClick={() =>
                      act(
                        listingPhotos.order(listingId, [
                          p.id,
                          ...list.filter((x) => x.id !== p.id).map((x) => x.id),
                        ])
                      )
                    }
                    className="rounded-md bg-black/60 p-1 text-white hover:bg-black/80"
                  >
                    <Star className="size-3.5" />
                  </button>
                )}
                <button
                  type="button"
                  aria-label="Remove photo"
                  onClick={() => act(listingPhotos.remove(p.id))}
                  className="rounded-md bg-black/60 p-1 text-white hover:bg-black/80"
                >
                  <X className="size-3.5" />
                </button>
              </div>
            </div>
            <input
              defaultValue={p.alt_text}
              aria-label="What the photo shows"
              onBlur={(e) => {
                const v = e.target.value.trim();
                if (v && v !== p.alt_text)
                  void act(listingPhotos.update(p.id, { alt_text: v }));
              }}
              className={cn(
                "w-full border-t border-line bg-transparent px-2 py-1.5 text-xs text-fg outline-none focus:bg-fill-2"
              )}
            />
          </li>
        ))}
      </ul>
    </div>
  );
}
