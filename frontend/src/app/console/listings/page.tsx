"use client";

// Console listing management — step one of the leasing pipeline: advertise a
// property, keep the ad current, and watch the pipeline retire it (Pending on
// conversion, Leased + unpublished when the lease is signed).

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api";
import type { ConsoleListing, Property } from "@/lib/types";
import { Badge, Card, statusTone } from "@/components/ui";
import { useAuth } from "@/lib/auth";
import { logError } from "@/lib/log";

export default function ListingsPage() {
  const { can } = useAuth();
  const manage = can("listing:write");

  const [listings, setListings] = useState<ConsoleListing[] | null>(null);
  const [properties, setProperties] = useState<Property[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);

  const load = useCallback(() => {
    api
      .consoleListings()
      .then((l) => {
        setListings(l);
        setError(null);
      })
      .catch((e) => setError(e.message));
    if (can("property:read")) {
      api
        .properties()
        .then(setProperties)
        .catch((e) => logError("failed to load properties", e));
    }
  }, [can]);

  useEffect(() => {
    load();
  }, [load]);

  async function togglePublic(l: ConsoleListing) {
    setBusy(l.id);
    try {
      await api.updateListing(l.id, { is_public: !l.is_public });
      load();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
    }
  }

  async function setStatus(l: ConsoleListing, status: string) {
    setBusy(l.id);
    try {
      await api.updateListing(l.id, { status });
      load();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
    }
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            Listings
          </h1>
          <p className="text-ink-3">
            What&apos;s advertised on your public website. Listings close out
            automatically as applications convert and leases get signed.
          </p>
        </div>
        {manage && !creating && (
          <button
            onClick={() => setCreating(true)}
            className="rounded-xl bg-accent px-4 py-2.5 text-sm font-bold text-on-accent"
          >
            New listing
          </button>
        )}
      </div>

      {error && <p className="text-bad">{error}</p>}

      {creating && (
        <CreateListingForm
          properties={properties}
          onDone={() => {
            setCreating(false);
            load();
          }}
          onCancel={() => setCreating(false)}
        />
      )}

      <Card className="overflow-hidden">
        <div className="divide-y divide-line">
          {listings?.map((l) => (
            <div
              key={l.id}
              className="flex flex-wrap items-center gap-3 px-5 py-3 text-sm"
            >
              <div className="min-w-0 flex-1">
                <div className="truncate font-semibold">{l.title}</div>
                <div className="text-xs text-ink-3">
                  {l.address}, {l.city} · {l.beds} bd / {l.baths} ba ·{" "}
                  {l.rent_label}/mo · available {l.available_on}
                </div>
              </div>
              <Badge tone={statusTone(l.status)}>{l.status}</Badge>
              <Badge tone={l.is_public ? "good" : "neutral"}>
                {l.is_public ? "public" : "hidden"}
              </Badge>
              {manage && (
                <>
                  <select
                    value={l.status}
                    onChange={(e) => setStatus(l, e.target.value)}
                    disabled={busy === l.id}
                    className="rounded-lg border border-line bg-surface px-2 py-1.5 text-xs"
                  >
                    {["Available", "New", "Pending", "Leased"].map((s) => (
                      <option key={s} value={s}>
                        {s}
                      </option>
                    ))}
                  </select>
                  <button
                    onClick={() => togglePublic(l)}
                    disabled={busy === l.id}
                    className="rounded-lg border border-line px-3 py-1.5 text-xs font-semibold disabled:opacity-50"
                  >
                    {l.is_public ? "Unpublish" : "Publish"}
                  </button>
                </>
              )}
            </div>
          ))}
          {listings?.length === 0 && (
            <div className="px-5 py-8 text-sm text-ink-3">
              Nothing advertised yet — create a listing to start taking
              applications.
            </div>
          )}
          {listings === null && !error && (
            <div className="px-5 py-8 text-sm text-ink-3">Loading…</div>
          )}
        </div>
      </Card>
    </div>
  );
}

function CreateListingForm({
  properties,
  onDone,
  onCancel,
}: {
  properties: Property[];
  onDone: () => void;
  onCancel: () => void;
}) {
  const [propertyId, setPropertyId] = useState("");
  const [title, setTitle] = useState("");
  const [rent, setRent] = useState("");
  const [availableOn, setAvailableOn] = useState("Now");
  const [description, setDescription] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    const dollars = parseFloat(rent);
    if (!propertyId || Number.isNaN(dollars) || dollars <= 0) {
      setError("Pick a property and enter a monthly rent.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await api.createListing(propertyId, {
        title: title.trim() || undefined,
        rent_cents: Math.round(dollars * 100),
        available_on: availableOn.trim() || undefined,
        description: description.trim() || undefined,
      });
      onDone();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Card className="p-5">
      <h2 className="mb-3 font-display text-lg font-bold">New listing</h2>
      {error && <p className="mb-2 text-sm text-bad">{error}</p>}
      <form onSubmit={submit} className="flex flex-wrap items-end gap-3">
        <label className="text-sm">
          <span className="mb-1 block text-ink-3">Property</span>
          <select
            value={propertyId}
            onChange={(e) => setPropertyId(e.target.value)}
            className="min-w-[220px] rounded-lg border border-line bg-surface px-3 py-2"
          >
            <option value="">Choose…</option>
            {properties.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name} — {p.address}
              </option>
            ))}
          </select>
        </label>
        <label className="flex-1 min-w-[180px] text-sm">
          <span className="mb-1 block text-ink-3">Title (optional)</span>
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="Defaults to the property name"
            className="w-full rounded-lg border border-line bg-surface px-3 py-2"
          />
        </label>
        <label className="text-sm">
          <span className="mb-1 block text-ink-3">Rent $/mo</span>
          <input
            value={rent}
            onChange={(e) => setRent(e.target.value)}
            inputMode="decimal"
            className="w-28 rounded-lg border border-line bg-surface px-3 py-2"
          />
        </label>
        <label className="text-sm">
          <span className="mb-1 block text-ink-3">Available</span>
          <input
            value={availableOn}
            onChange={(e) => setAvailableOn(e.target.value)}
            className="w-28 rounded-lg border border-line bg-surface px-3 py-2"
          />
        </label>
        <label className="w-full text-sm">
          <span className="mb-1 block text-ink-3">Description</span>
          <input
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            className="w-full rounded-lg border border-line bg-surface px-3 py-2"
          />
        </label>
        <button
          type="submit"
          disabled={busy}
          className="rounded-lg bg-accent px-4 py-2 font-semibold text-on-accent disabled:opacity-50"
        >
          {busy ? "Creating…" : "Create & publish"}
        </button>
        <button
          type="button"
          onClick={onCancel}
          className="rounded-lg border border-line px-4 py-2 font-semibold"
        >
          Cancel
        </button>
      </form>
    </Card>
  );
}
