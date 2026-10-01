"use client";

// Photos on a listing: upload, describe (alt text is required), caption,
// reorder and remove. The first is the hero on the public page, the share
// image and the structured-data image.

import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { api, request } from "@/lib/api";
import { Button } from "@/components/ui";

export interface ListingPhotoRow {
  id: string;
  document_id: string;
  alt_text: string;
  caption: string | null;
  position: number;
  public_url: string;
  preview_url: string | null;
}

const field =
  "w-full rounded-lg border border-line bg-surface px-2 py-1.5 text-sm outline-none focus:border-accent";

const photosApi = {
  list: (listingId: string) =>
    request<ListingPhotoRow[]>(`/listings/${listingId}/photos`, { auth: true }),
  add: (listingId: string, document_id: string, alt_text: string) =>
    request<ListingPhotoRow[]>(`/listings/${listingId}/photos`, {
      method: "POST",
      auth: true,
      body: { document_id, alt_text },
    }),
  update: (id: string, body: { alt_text?: string; caption?: string }) =>
    request<ListingPhotoRow[]>(`/listing-photos/${id}`, {
      method: "PATCH",
      auth: true,
      body,
    }),
  order: (listingId: string, ids: string[]) =>
    request<ListingPhotoRow[]>(`/listings/${listingId}/photos/order`, {
      method: "PUT",
      auth: true,
      body: { ids },
    }),
  remove: (id: string) =>
    request<ListingPhotoRow[]>(`/listing-photos/${id}`, {
      method: "DELETE",
      auth: true,
    }),
};

/** A starting alt text from the file name: "front-porch_2.jpg" → "Front porch 2". */
export function altFromFilename(name: string): string {
  const base = name
    .replace(/\.[a-z0-9]+$/i, "")
    .replace(/[-_]+/g, " ")
    .trim();
  return base ? base.charAt(0).toUpperCase() + base.slice(1) : "";
}

export function ListingPhotos({
  listingId,
  manage,
}: {
  listingId: string;
  manage: boolean;
}) {
  const [rows, setRows] = useState<ListingPhotoRow[] | null>(null);
  const [busy, setBusy] = useState(false);
  const fileInput = useRef<HTMLInputElement>(null);

  const load = useCallback(() => {
    photosApi
      .list(listingId)
      .then(setRows)
      .catch((e) => toast.error(e.message));
  }, [listingId]);
  useEffect(load, [load]);

  async function run(fn: () => Promise<ListingPhotoRow[]>) {
    setBusy(true);
    try {
      setRows(await fn());
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Request failed");
    } finally {
      setBusy(false);
    }
  }

  async function upload(files: FileList) {
    setBusy(true);
    try {
      for (const file of Array.from(files)) {
        if (!file.type.startsWith("image/")) {
          toast.error(`${file.name} isn't an image`);
          continue;
        }
        const alt = window.prompt(
          `Describe ${file.name} for people who can't see it`,
          altFromFilename(file.name)
        );
        if (!alt?.trim()) continue;
        const doc = await api.uploadDocument(
          {
            owner_type: "listing",
            owner_id: listingId,
            filename: file.name,
            mime_type: file.type,
          },
          file
        );
        setRows(await photosApi.add(listingId, doc.id, alt.trim()));
      }
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Upload failed");
    } finally {
      setBusy(false);
      if (fileInput.current) fileInput.current.value = "";
    }
  }

  const move = (i: number, by: number) => {
    if (!rows) return;
    const ids = rows.map((r) => r.id);
    const j = i + by;
    if (j < 0 || j >= ids.length) return;
    [ids[i], ids[j]] = [ids[j], ids[i]];
    void run(() => photosApi.order(listingId, ids));
  };

  if (!rows) return <p className="text-sm text-ink-3">Loading photos…</p>;

  return (
    <div className="space-y-3">
      {rows.length === 0 && (
        <p className="text-sm text-ink-3">
          No photos yet. The first one you add is the main picture.
        </p>
      )}
      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {rows.map((r, i) => (
          <div
            key={r.id}
            className="overflow-hidden rounded-xl border border-line bg-surface"
          >
            <div className="relative aspect-[4/3] bg-surface-2">
              {r.preview_url && (
                // eslint-disable-next-line @next/next/no-img-element
                <img
                  src={r.preview_url}
                  alt={r.alt_text}
                  className="h-full w-full object-cover"
                />
              )}
              {i === 0 && (
                <span className="absolute left-2 top-2 rounded-md bg-accent px-2 py-0.5 text-xs font-bold text-on-accent">
                  Main photo
                </span>
              )}
            </div>
            <div className="space-y-2 p-2">
              <input
                className={field}
                aria-label="Alt text"
                defaultValue={r.alt_text}
                disabled={!manage || busy}
                onBlur={(e) => {
                  const v = e.target.value.trim();
                  if (v && v !== r.alt_text)
                    void run(() => photosApi.update(r.id, { alt_text: v }));
                }}
              />
              <input
                className={field}
                aria-label="Caption"
                placeholder="Caption (optional)"
                defaultValue={r.caption ?? ""}
                disabled={!manage || busy}
                onBlur={(e) => {
                  const v = e.target.value.trim();
                  if (v !== (r.caption ?? ""))
                    void run(() => photosApi.update(r.id, { caption: v }));
                }}
              />
              {manage && (
                <div className="flex items-center justify-between text-xs font-semibold">
                  <div className="flex gap-2">
                    <button
                      disabled={busy || i === 0}
                      onClick={() => move(i, -1)}
                      className="text-ink-2 disabled:opacity-30"
                      aria-label="Move earlier"
                    >
                      ← Earlier
                    </button>
                    <button
                      disabled={busy || i === rows.length - 1}
                      onClick={() => move(i, 1)}
                      className="text-ink-2 disabled:opacity-30"
                      aria-label="Move later"
                    >
                      Later →
                    </button>
                  </div>
                  <button
                    disabled={busy}
                    onClick={() => {
                      if (confirm("Take this photo off the listing?"))
                        void run(() => photosApi.remove(r.id));
                    }}
                    className="text-bad"
                  >
                    Remove
                  </button>
                </div>
              )}
            </div>
          </div>
        ))}
      </div>
      {manage && (
        <>
          <input
            ref={fileInput}
            type="file"
            accept="image/*"
            multiple
            className="hidden"
            onChange={(e) => e.target.files && upload(e.target.files)}
          />
          <Button
            variant="outline"
            disabled={busy}
            onClick={() => fileInput.current?.click()}
          >
            {busy ? "Working…" : "Add photos"}
          </Button>
        </>
      )}
    </div>
  );
}
