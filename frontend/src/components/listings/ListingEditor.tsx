"use client";

// Edit what a listing says: what the website and the portals show.

import { useState } from "react";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { parseCents } from "@/lib/servicedesk";
import type { ConsoleListing } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { ListingPhotos } from "@/components/listings/ListingPhotos";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { cn } from "@/lib/utils";

const field =
  "w-full rounded-xl border border-line bg-surface px-3 py-2 text-[13px] text-fg outline-none focus:border-accent";

const STATUSES = ["Available", "New", "Pending", "Leased"];

export function ListingEditor({
  listing: l,
  issues,
  onClose,
  onSaved,
  onPhotos,
}: {
  listing: ConsoleListing;
  issues: string[];
  onClose: () => void;
  onSaved: () => void;
  /** Photos are saved as they're added; the list behind refreshes. */
  onPhotos?: () => void;
}) {
  const [d, setD] = useState({
    title: l.title,
    rent: (l.rent_cents / 100).toFixed(2),
    beds: String(l.beds),
    baths: String(l.baths),
    sqft: l.sqft ? String(l.sqft) : "",
    available_on: l.available_on,
    description: l.description,
    state: l.state,
    postal_code: l.postal_code,
    status: l.status,
    is_public: l.is_public,
    syndicate: l.syndicate,
  });
  const [busy, setBusy] = useState(false);
  const set = <K extends keyof typeof d>(k: K, v: (typeof d)[K]) =>
    setD((x) => ({ ...x, [k]: v }));
  const words = d.description.trim().split(/\s+/).filter(Boolean).length;

  async function save() {
    const rent = parseCents(d.rent);
    if (!rent || rent <= 0) {
      toast.error("Enter the rent.");
      return;
    }
    setBusy(true);
    try {
      await api.updateListing(l.id, {
        title: d.title.trim(),
        rent_cents: rent,
        beds: Math.max(0, Number(d.beds) || 0),
        baths: Math.max(0, Number(d.baths) || 0),
        sqft: Math.max(0, Number(d.sqft) || 0),
        available_on: d.available_on.trim(),
        description: d.description,
        state: d.state.trim(),
        postal_code: d.postal_code.trim(),
        status: d.status,
        is_public: d.is_public,
        syndicate: d.syndicate,
      });
      toast.success("Listing saved");
      onSaved();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't save it");
      setBusy(false);
    }
  }

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-h-[90vh] max-w-2xl overflow-y-auto">
        <DialogTitle className="text-[17px] font-semibold">
          {l.title}
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          {l.address}, {l.city}
        </DialogDescription>
        {issues.length > 0 && (
          <ul className="mt-3 space-y-1 rounded-xl border border-warn/30 bg-warn/10 px-3 py-2 text-[13px] text-fg">
            {issues.map((i) => (
              <li key={i}>{i}</li>
            ))}
          </ul>
        )}
        <div className="mt-4">
          <ListingPhotos
            listingId={l.id}
            title={l.title}
            onChange={() => onPhotos?.()}
          />
        </div>
        <div className="mt-4 grid gap-3 sm:grid-cols-2">
          <label className="space-y-1 sm:col-span-2">
            <span className="text-xs text-fg-3">Headline</span>
            <input
              className={field}
              value={d.title}
              onChange={(e) => set("title", e.target.value)}
            />
          </label>
          <label className="space-y-1">
            <span className="text-xs text-fg-3">Rent a month</span>
            <input
              className={field}
              inputMode="decimal"
              value={d.rent}
              onChange={(e) => set("rent", e.target.value)}
            />
          </label>
          <label className="space-y-1">
            <span className="text-xs text-fg-3">Available</span>
            <input
              className={field}
              placeholder="Now, Nov 15, 2026-11-15"
              value={d.available_on}
              onChange={(e) => set("available_on", e.target.value)}
            />
          </label>
          <div className="grid grid-cols-3 gap-2 sm:col-span-2">
            {(
              [
                ["beds", "Beds"],
                ["baths", "Baths"],
                ["sqft", "Sq ft"],
              ] as const
            ).map(([k, label]) => (
              <label key={k} className="space-y-1">
                <span className="text-xs text-fg-3">{label}</span>
                <input
                  className={field}
                  inputMode="numeric"
                  value={d[k]}
                  onChange={(e) =>
                    set(k, e.target.value.replace(/[^0-9]/g, ""))
                  }
                />
              </label>
            ))}
          </div>
          <label className="space-y-1">
            <span className="text-xs text-fg-3">State</span>
            <input
              className={field}
              maxLength={2}
              placeholder="OR"
              value={d.state}
              onChange={(e) => set("state", e.target.value.toUpperCase())}
            />
          </label>
          <label className="space-y-1">
            <span className="text-xs text-fg-3">ZIP</span>
            <input
              className={field}
              inputMode="numeric"
              value={d.postal_code}
              onChange={(e) => set("postal_code", e.target.value)}
            />
          </label>
          <label className="space-y-1 sm:col-span-2">
            <span className="flex justify-between text-xs text-fg-3">
              Description
              <span className={cn(words < 30 && "text-warn")}>
                {words} words
              </span>
            </span>
            <textarea
              className={cn(field, "min-h-32")}
              value={d.description}
              onChange={(e) => set("description", e.target.value)}
            />
          </label>
          <label className="space-y-1">
            <span className="text-xs text-fg-3">Status</span>
            <select
              className={field}
              value={d.status}
              onChange={(e) => set("status", e.target.value)}
            >
              {STATUSES.map((s) => (
                <option key={s}>{s}</option>
              ))}
            </select>
          </label>
          <div className="space-y-2 self-end pb-1 text-[13px] text-fg-2">
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={d.is_public}
                onChange={(e) => set("is_public", e.target.checked)}
              />
              On your website
            </label>
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={d.syndicate}
                onChange={(e) => set("syndicate", e.target.checked)}
              />
              Send to the rental portals
            </label>
          </div>
        </div>
        <div className="mt-5 flex justify-end gap-2">
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={save} disabled={busy}>
            Save
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
