"use client";

// Business profile and Google reviews. Clients edit this themselves; Vantedge
// staff working in a workspace (on a call) edit the same record, and those
// changes are flagged as support edits in the audit trail.

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { toast } from "sonner";
import { api } from "@/lib/api";
import {
  business,
  MAPS_KEY,
  type Business,
  type BusinessInput,
  type PlaceCandidate,
  type PlaceInfo,
} from "@/lib/business";
import { useAuth } from "@/lib/auth";
import { Badge, Button, Card } from "@/components/ui";
import { Stars } from "@/components/Stars";

const field =
  "w-full rounded-xl border border-line bg-surface px-3 py-2 text-sm text-ink";

const TEXT: [keyof BusinessInput, string, string][] = [
  ["business_name", "Business name", "Northwind Rentals"],
  ["phone", "Phone", "(555) 010-0100"],
  ["email", "Email", "hello@example.com"],
  ["website", "Website", "https://"],
  ["address", "Address", "Street, city, state"],
  ["facebook_url", "Facebook", "https://facebook.com/…"],
  ["instagram_url", "Instagram", "https://instagram.com/…"],
  ["yelp_url", "Yelp", "https://yelp.com/biz/…"],
  ["nextdoor_url", "Nextdoor", "https://nextdoor.com/pages/…"],
];

export default function BusinessSettingsPage() {
  const { can, user } = useAuth();
  const allowed = can("integrations:manage");
  const [biz, setBiz] = useState<Business | null>(null);
  const [form, setForm] = useState<BusinessInput>({});
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const apply = useCallback((b: Business) => {
    setBiz(b);
    setForm({
      business_name: b.business_name ?? "",
      phone: b.phone ?? "",
      email: b.email ?? "",
      website: b.website ?? "",
      address: b.address ?? "",
      hours: b.hours ?? "",
      description: b.description ?? "",
      facebook_url: b.facebook_url ?? "",
      instagram_url: b.instagram_url ?? "",
      yelp_url: b.yelp_url ?? "",
      nextdoor_url: b.nextdoor_url ?? "",
      google_review_url: b.google_review_url ?? "",
      show_reviews: b.show_reviews,
      min_rating: b.min_rating,
      max_reviews: b.max_reviews,
      refresh_minutes: b.refresh_minutes,
    });
  }, []);

  useEffect(() => {
    if (!allowed) return;
    business
      .get()
      .then(apply)
      .catch((e: Error) => setError(e.message));
  }, [allowed, apply]);

  const save = async (body: BusinessInput = form, ok = "Saved") => {
    setSaving(true);
    try {
      apply(await business.save(body));
      toast.success(ok);
    } catch (e) {
      toast.error((e as Error).message);
    } finally {
      setSaving(false);
    }
  };

  if (!allowed)
    return (
      <Card className="p-6 text-ink-2">
        You need the <span className="font-mono">integrations:manage</span>{" "}
        permission to edit the business profile.
      </Card>
    );
  if (error) return <p className="text-sm text-bad">{error}</p>;
  if (!biz) return <p className="text-sm text-ink-3">Loading…</p>;

  return (
    <div className="space-y-6">
      <div>
        <Link href="/console/settings" className="text-xs text-ink-3">
          ← Settings
        </Link>
        <h1 className="font-display text-2xl font-bold">
          Business profile and Google reviews
        </h1>
        {user?.is_platform_staff && (
          <p className="mt-2 rounded-xl bg-info-soft px-3 py-2 text-sm text-info">
            You are editing this workspace as Vantedge staff. Your changes are
            recorded in the audit trail as support edits.
          </p>
        )}
      </div>

      <Card className="space-y-4 p-5">
        <h2 className="font-display text-lg font-bold">Your business</h2>
        <div className="grid gap-3 md:grid-cols-2">
          {TEXT.map(([k, label, ph]) => (
            <label key={k} className="text-xs font-semibold text-ink-3">
              {label}
              <input
                className={`${field} mt-1`}
                placeholder={ph}
                value={(form[k] as string) ?? ""}
                onChange={(e) => setForm({ ...form, [k]: e.target.value })}
              />
            </label>
          ))}
        </div>
        <label className="block text-xs font-semibold text-ink-3">
          Hours
          <textarea
            className={`${field} mt-1`}
            rows={3}
            placeholder={"Mon to Fri 9 to 5\nSat 10 to 2"}
            value={form.hours ?? ""}
            onChange={(e) => setForm({ ...form, hours: e.target.value })}
          />
        </label>
        <label className="block text-xs font-semibold text-ink-3">
          About
          <textarea
            className={`${field} mt-1`}
            rows={3}
            value={form.description ?? ""}
            onChange={(e) => setForm({ ...form, description: e.target.value })}
          />
        </label>
        <Button onClick={() => save()} disabled={saving}>
          Save profile
        </Button>
      </Card>

      <GoogleCard
        biz={biz}
        form={form}
        setForm={setForm}
        save={save}
        onKey={() => business.get().then(apply)}
      />
    </div>
  );
}

function GoogleCard({
  biz,
  form,
  setForm,
  save,
  onKey,
}: {
  biz: Business;
  form: BusinessInput;
  setForm: (f: BusinessInput) => void;
  save: (body?: BusinessInput, ok?: string) => Promise<void>;
  onKey: () => void;
}) {
  const [query, setQuery] = useState(biz.business_name ?? "");
  const [results, setResults] = useState<PlaceCandidate[] | null>(null);
  const [place, setPlace] = useState<PlaceInfo | null>(null);
  const [placeError, setPlaceError] = useState<string | null>(null);
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!biz.google_place_id) return;
    business
      .place()
      .then((p) => {
        setPlace(p);
        setPlaceError(null);
      })
      .catch((e: Error) => setPlaceError(e.message));
  }, [biz.google_place_id]);

  const find = async () => {
    setBusy(true);
    try {
      setResults(await business.search(query));
    } catch (e) {
      toast.error((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  const saveKey = async () => {
    try {
      await api.setIntegrationSecret(MAPS_KEY, key.trim());
      setKey("");
      toast.success("Google key saved");
      onKey();
    } catch (e) {
      toast.error((e as Error).message);
    }
  };

  return (
    <Card className="space-y-4 p-5">
      <div className="flex flex-wrap items-center gap-2">
        <h2 className="font-display text-lg font-bold">Google reviews</h2>
        {biz.google_live ? (
          <Badge tone="good">Live</Badge>
        ) : (
          <Badge tone="warn">Sample data</Badge>
        )}
      </div>

      {!biz.google_key_set && (
        <p className="rounded-xl bg-warn-soft px-3 py-2 text-sm text-warn">
          Add a Google Maps key (Places API New enabled) so Vantedge can find
          your business and show its reviews. Until then the screen shows sample
          reviews.
        </p>
      )}
      <div className="flex flex-wrap items-end gap-2">
        <label className="min-w-64 flex-1 text-xs font-semibold text-ink-3">
          Google Maps API key
          <input
            className={`${field} mt-1`}
            type="password"
            autoComplete="off"
            placeholder={
              biz.google_key_set ? "A key is saved. Paste to replace." : "AIza…"
            }
            value={key}
            onChange={(e) => setKey(e.target.value)}
          />
        </label>
        <Button variant="outline" onClick={saveKey} disabled={!key.trim()}>
          Save key
        </Button>
      </div>

      {biz.google_place_id ? (
        <div className="rounded-xl border border-line p-4">
          <div className="flex flex-wrap items-center gap-3">
            <div className="min-w-0 flex-1">
              <div className="font-semibold">
                {place?.name || biz.google_place_name || "Your business"}
              </div>
              {place && (
                <div className="flex items-center gap-2 text-sm text-ink-2">
                  {place.rating !== null && <Stars rating={place.rating} />}
                  {place.rating?.toFixed(1)} · {place.count} reviews
                </div>
              )}
              {placeError && (
                <div className="text-sm text-bad">{placeError}</div>
              )}
            </div>
            <Button
              variant="outline"
              onClick={() =>
                business
                  .place(true)
                  .then((p) => {
                    setPlace(p);
                    toast.success("Refreshed from Google");
                  })
                  .catch((e: Error) => toast.error(e.message))
              }
            >
              Refresh
            </Button>
            <Button
              variant="ghost"
              onClick={() =>
                save({ google_place_id: "" }, "Business unlinked from Google")
              }
            >
              Change
            </Button>
          </div>
          {biz.review_link && (
            <p className="mt-2 break-all text-xs text-ink-3">
              Review link: {biz.review_link}
            </p>
          )}
        </div>
      ) : (
        <div className="space-y-2">
          <div className="flex gap-2">
            <input
              className={field}
              placeholder="Business name and town, as on Google Maps"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && find()}
            />
            <Button onClick={find} disabled={busy || query.trim().length < 2}>
              Find
            </Button>
          </div>
          {results?.length === 0 && (
            <p className="text-sm text-ink-3">
              Nothing found. Try the name exactly as it shows on Google Maps.
            </p>
          )}
          {results?.map((r) => (
            <div
              key={r.place_id}
              className="flex items-center gap-3 rounded-xl border border-line p-3"
            >
              <div className="min-w-0 flex-1">
                <div className="font-semibold">{r.name}</div>
                <div className="text-xs text-ink-3">{r.address}</div>
                {r.rating !== null && (
                  <div className="text-xs text-ink-2">
                    {r.rating.toFixed(1)} · {r.count} reviews
                  </div>
                )}
              </div>
              <Button
                onClick={() =>
                  save(
                    { google_place_id: r.place_id, google_place_name: r.name },
                    `${r.name} is your business on Google`
                  )
                }
              >
                This is it
              </Button>
            </div>
          ))}
        </div>
      )}

      <div className="grid gap-3 border-t border-line pt-4 md:grid-cols-4">
        <label className="flex items-center gap-2 text-sm font-semibold">
          <input
            type="checkbox"
            checked={form.show_reviews ?? true}
            onChange={(e) =>
              setForm({ ...form, show_reviews: e.target.checked })
            }
          />
          Show on the website
        </label>
        <label className="text-xs font-semibold text-ink-3">
          Lowest rating shown
          <select
            className={`${field} mt-1`}
            value={form.min_rating ?? 4}
            onChange={(e) =>
              setForm({ ...form, min_rating: Number(e.target.value) })
            }
          >
            {[1, 2, 3, 4, 5].map((n) => (
              <option key={n} value={n}>
                {n} star{n > 1 ? "s" : ""} and up
              </option>
            ))}
          </select>
        </label>
        <label className="text-xs font-semibold text-ink-3">
          Reviews shown
          <select
            className={`${field} mt-1`}
            value={form.max_reviews ?? 5}
            onChange={(e) =>
              setForm({ ...form, max_reviews: Number(e.target.value) })
            }
          >
            {[1, 2, 3, 4, 5].map((n) => (
              <option key={n}>{n}</option>
            ))}
          </select>
        </label>
        <label className="text-xs font-semibold text-ink-3">
          Refresh every (minutes)
          <input
            className={`${field} mt-1`}
            type="number"
            min={5}
            max={1440}
            value={form.refresh_minutes ?? 360}
            onChange={(e) =>
              setForm({ ...form, refresh_minutes: Number(e.target.value) })
            }
          />
        </label>
      </div>
      <label className="block text-xs font-semibold text-ink-3">
        Review link override (optional)
        <input
          className={`${field} mt-1`}
          placeholder="https://g.page/r/…/review"
          value={form.google_review_url ?? ""}
          onChange={(e) =>
            setForm({ ...form, google_review_url: e.target.value })
          }
        />
      </label>
      <p className="text-xs text-ink-3">
        Google&apos;s terms allow keeping only the place id. Review text is
        fetched live, held briefly in memory, and never saved.
      </p>
      <Button
        onClick={() =>
          save(
            {
              show_reviews: form.show_reviews,
              min_rating: form.min_rating,
              max_reviews: form.max_reviews,
              refresh_minutes: form.refresh_minutes,
              google_review_url: form.google_review_url,
            },
            "Review settings saved"
          )
        }
      >
        Save review settings
      </Button>
    </Card>
  );
}
