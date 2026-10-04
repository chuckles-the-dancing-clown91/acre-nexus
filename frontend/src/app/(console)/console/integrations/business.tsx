"use client";

// The business profile and its Google reviews. Clients edit this themselves;
// Vantedge staff working in a workspace edit the same record, and those edits
// are marked as support edits in the audit trail.

import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Check,
  ExternalLink,
  Link2Off,
  MapPin,
  RefreshCw,
  Search,
  Star,
} from "lucide-react";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import {
  business,
  MAPS_KEY,
  type Business,
  type BusinessInput,
  type PlaceCandidate,
} from "@/lib/business";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Field, fieldClass, Input, Label } from "@/components/ui/input";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";
import { errorText, Stars, Switch } from "./bits";

export const BUSINESS_KEY = ["business-profile"];

/** The business profile, for anyone with integrations:manage. */
export function useBusiness() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  return useQuery({
    queryKey: BUSINESS_KEY,
    queryFn: business.get,
    enabled: scoped && can("integrations:manage"),
  });
}

/** Save part of the profile; the cache takes the server's answer. */
export function useSaveBusiness() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ body }: { body: BusinessInput; ok?: string }) =>
      business.save(body),
    onSuccess: (b, { ok }) => {
      qc.setQueryData(BUSINESS_KEY, b);
      toast.success(ok ?? "Saved");
    },
    onError: (e) => toast.error(errorText(e, "Couldn't save")),
  });
}

/** Loading and error states around something that needs the profile. */
export function WithBusiness({
  children,
}: {
  children: (b: Business) => React.ReactNode;
}) {
  const q = useBusiness();
  if (q.isLoading) return <Skeleton className="h-64 rounded-2xl" />;
  if (q.error)
    return (
      <Panel className="border-bad/30 p-4 text-[13px] text-bad">
        Couldn&apos;t load the business profile: {q.error.message}
      </Panel>
    );
  if (!q.data) return null;
  return <>{children(q.data)}</>;
}

function StaffNote() {
  const { user } = useAuth();
  if (!user?.is_platform_staff) return null;
  return (
    <Panel className="border-info/30 p-4 text-[13px] text-info">
      You are editing this workspace as Vantedge staff. Your changes are
      recorded in the audit trail as support edits.
    </Panel>
  );
}

const TEXT: [keyof BusinessInput, string, string][] = [
  ["business_name", "Business name", "Northwind Rentals"],
  ["phone", "Phone", "(555) 010-0100"],
  ["email", "Email", "hello@example.com"],
  ["website", "Website", "https://"],
  ["address", "Address", "Street, city, state"],
];

const SOCIAL: [keyof BusinessInput, string, string][] = [
  ["facebook_url", "Facebook", "https://facebook.com/…"],
  ["instagram_url", "Instagram", "https://instagram.com/…"],
  ["yelp_url", "Yelp", "https://yelp.com/biz/…"],
  ["nextdoor_url", "Nextdoor", "https://nextdoor.com/pages/…"],
];

const textarea = cn(fieldClass, "block w-full py-2.5 text-sm");

export function BusinessTab() {
  return (
    <div className="space-y-6">
      <StaffNote />
      <WithBusiness>
        {(b) => <BusinessForm key={b.updated_at ?? "new"} biz={b} />}
      </WithBusiness>
    </div>
  );
}

function BusinessForm({ biz }: { biz: Business }) {
  const save = useSaveBusiness();
  const fields = [...TEXT, ...SOCIAL].map(([k]) => k);
  const [form, setForm] = useState<BusinessInput>(() => {
    const f: Record<string, string> = {};
    for (const k of [...fields, "hours", "description"] as const)
      f[k] = (biz[k as keyof Business] as string | null) ?? "";
    return f as BusinessInput;
  });
  const set = (k: keyof BusinessInput, v: string) =>
    setForm((f) => ({ ...f, [k]: v }));

  return (
    <form
      className="space-y-6"
      onSubmit={(e) => {
        e.preventDefault();
        save.mutate({ body: form, ok: "Business profile saved" });
      }}
    >
      <Panel>
        <PanelHeader
          title="Your business"
          description="Used on your website, in its search listing and in emails."
        />
        <div className="grid gap-4 p-5 md:grid-cols-2">
          {TEXT.map(([k, label, ph]) => (
            <Field key={k} label={label}>
              {(p) => (
                <Input
                  {...p}
                  placeholder={ph}
                  value={(form[k] as string) ?? ""}
                  onChange={(e) => set(k, e.target.value)}
                />
              )}
            </Field>
          ))}
          <div className="space-y-1.5 md:col-span-2">
            <Label htmlFor="biz-hours">Hours</Label>
            <textarea
              id="biz-hours"
              className={textarea}
              rows={3}
              placeholder={"Mon to Fri 9 to 5\nSat 10 to 2"}
              value={form.hours ?? ""}
              onChange={(e) => set("hours", e.target.value)}
            />
          </div>
          <div className="space-y-1.5 md:col-span-2">
            <Label htmlFor="biz-about">About</Label>
            <textarea
              id="biz-about"
              className={textarea}
              rows={3}
              value={form.description ?? ""}
              onChange={(e) => set("description", e.target.value)}
            />
          </div>
        </div>
      </Panel>
      <Panel>
        <PanelHeader
          title="Social profiles"
          description="Linked from your website's footer."
        />
        <div className="grid gap-4 p-5 md:grid-cols-2">
          {SOCIAL.map(([k, label, ph]) => (
            <Field key={k} label={label}>
              {(p) => (
                <Input
                  {...p}
                  placeholder={ph}
                  value={(form[k] as string) ?? ""}
                  onChange={(e) => set(k, e.target.value)}
                />
              )}
            </Field>
          ))}
        </div>
      </Panel>
      <div className="flex justify-end">
        <Button type="submit" loading={save.isPending}>
          {!save.isPending && <Check />}
          Save profile
        </Button>
      </div>
    </form>
  );
}

// ---- Google reviews ----

export function ReviewsTab() {
  return (
    <div className="space-y-6">
      <StaffNote />
      <WithBusiness>
        {(b) => (
          <>
            <GoogleKey biz={b} />
            <GooglePlace biz={b} />
            <ReviewSettings key={b.updated_at ?? "new"} biz={b} />
          </>
        )}
      </WithBusiness>
    </div>
  );
}

function GoogleKey({ biz }: { biz: Business }) {
  const qc = useQueryClient();
  const [key, setKey] = useState("");
  const saveKey = useMutation({
    mutationFn: () => api.setIntegrationSecret(MAPS_KEY, key.trim()),
    onSuccess: () => {
      setKey("");
      toast.success("Google key saved");
      qc.invalidateQueries({ queryKey: BUSINESS_KEY });
      qc.invalidateQueries({ queryKey: ["integration-secrets"] });
    },
    onError: (e) => toast.error(errorText(e, "Couldn't save the key")),
  });
  return (
    <Panel>
      <PanelHeader
        title="Google Maps key"
        description="Needs the Places API (New) turned on. Without it the reviews shown are samples."
        action={
          <div className="flex gap-1.5">
            <Badge tone={biz.google_key_set ? "good" : "warn"}>
              {biz.google_key_set ? "Key saved" : "No key"}
            </Badge>
            <Badge tone={biz.google_live ? "good" : "neutral"}>
              {biz.google_live ? "Live" : "Sample data"}
            </Badge>
          </div>
        }
      />
      <form
        className="flex flex-col gap-2 p-5 sm:flex-row"
        onSubmit={(e) => {
          e.preventDefault();
          if (key.trim()) saveKey.mutate();
        }}
      >
        <Input
          aria-label="Google Maps API key"
          type="password"
          autoComplete="off"
          className="font-mono"
          placeholder={
            biz.google_key_set
              ? "A key is saved. Paste one to replace it."
              : "AIza…"
          }
          value={key}
          onChange={(e) => setKey(e.target.value)}
        />
        <Button
          type="submit"
          variant="secondary"
          size="lg"
          disabled={!key.trim()}
          loading={saveKey.isPending}
        >
          Save key
        </Button>
      </form>
    </Panel>
  );
}

function GooglePlace({ biz }: { biz: Business }) {
  const save = useSaveBusiness();
  const qc = useQueryClient();
  const linked = !!biz.google_place_id;
  const place = useQuery({
    queryKey: ["business-place", biz.google_place_id],
    queryFn: () => business.place(),
    enabled: linked,
    retry: false,
  });
  const refresh = useMutation({
    mutationFn: () => business.place(true),
    onSuccess: (p) => {
      qc.setQueryData(["business-place", biz.google_place_id], p);
      toast.success("Refreshed from Google");
    },
    onError: (e) => toast.error(errorText(e, "Couldn't refresh")),
  });
  const [query, setQuery] = useState(biz.business_name ?? "");
  const [results, setResults] = useState<PlaceCandidate[] | null>(null);
  const find = useMutation({
    mutationFn: () => business.search(query.trim()),
    onSuccess: setResults,
    onError: (e) => toast.error(errorText(e, "Search failed")),
  });

  const p = place.data;
  return (
    <Panel>
      <PanelHeader
        title="Your business on Google"
        description="Link the Google Maps listing whose reviews you want to show."
        action={
          linked && (
            <div className="flex gap-2">
              <Button
                size="sm"
                variant="secondary"
                loading={refresh.isPending}
                onClick={() => refresh.mutate()}
              >
                {!refresh.isPending && <RefreshCw />}
                Refresh
              </Button>
              <Button
                size="sm"
                variant="ghost"
                disabled={save.isPending}
                onClick={() => {
                  setResults(null);
                  save.mutate({
                    body: { google_place_id: "" },
                    ok: "Unlinked from Google",
                  });
                }}
              >
                <Link2Off />
                Change
              </Button>
            </div>
          )
        }
      />
      <div className="p-5">
        {linked ? (
          <div className="space-y-4">
            <div className="flex items-start gap-3">
              <span className="flex size-10 shrink-0 items-center justify-center rounded-xl border border-line bg-fill text-fg-2">
                <MapPin className="size-4" />
              </span>
              <div className="min-w-0 flex-1">
                <div className="text-[14px] font-medium text-fg">
                  {p?.name || biz.google_place_name || "Your business"}
                </div>
                {p && (
                  <div className="mt-0.5 flex flex-wrap items-center gap-2 text-[13px] text-fg-2">
                    {p.rating !== null && <Stars rating={p.rating} />}
                    {p.rating !== null && (
                      <span className="figure">{p.rating.toFixed(1)}</span>
                    )}
                    <span className="text-fg-3">
                      {p.count} review{p.count === 1 ? "" : "s"}
                    </span>
                    {p.simulated && <Badge tone="neutral">Sample</Badge>}
                    {p.maps_url && (
                      <a
                        href={p.maps_url}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="inline-flex items-center gap-1 text-accent hover:underline"
                      >
                        Open in Maps
                        <ExternalLink className="size-3" />
                      </a>
                    )}
                  </div>
                )}
                {place.error && (
                  <p className="mt-1 text-[13px] text-bad">
                    {place.error.message}
                  </p>
                )}
                {biz.review_link && (
                  <p className="mt-1 text-[12px] break-all text-fg-3">
                    Review link: {biz.review_link}
                  </p>
                )}
              </div>
            </div>
            {place.isLoading && <Skeleton className="h-24" />}
            {p && p.reviews.length > 0 && (
              <div className="grid gap-3 md:grid-cols-2">
                {p.reviews.map((r, i) => (
                  <figure
                    key={`${r.author}-${i}`}
                    className="rounded-xl border border-line bg-fill/50 p-4"
                  >
                    <Stars rating={r.rating} size={12} />
                    <blockquote className="mt-1.5 line-clamp-4 text-[13px] text-fg-2">
                      {r.text}
                    </blockquote>
                    <figcaption className="mt-2 text-xs text-fg-3">
                      {r.author}
                      {r.when ? ` · ${r.when}` : ""}
                    </figcaption>
                  </figure>
                ))}
              </div>
            )}
          </div>
        ) : (
          <div className="space-y-3">
            <form
              className="flex flex-col gap-2 sm:flex-row"
              onSubmit={(e) => {
                e.preventDefault();
                if (query.trim().length >= 2) find.mutate();
              }}
            >
              <div className="relative flex-1">
                <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-fg-3" />
                <Input
                  aria-label="Business name and town"
                  className="pl-9"
                  placeholder="Business name and town, as on Google Maps"
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                />
              </div>
              <Button
                type="submit"
                size="lg"
                disabled={query.trim().length < 2}
                loading={find.isPending}
              >
                Find
              </Button>
            </form>
            {results?.length === 0 && (
              <EmptyState
                icon={<Search />}
                title="Nothing found"
                description="Try the name exactly as it shows on Google Maps."
              />
            )}
            {results && results.length > 0 && (
              <ul className="divide-y divide-line rounded-xl border border-line">
                {results.map((r) => (
                  <li
                    key={r.place_id}
                    className="flex flex-col gap-2 px-4 py-3 sm:flex-row sm:items-center"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="text-[14px] font-medium text-fg">
                        {r.name}
                      </div>
                      <div className="text-[12px] text-fg-3">{r.address}</div>
                      {r.rating !== null && (
                        <div className="mt-0.5 flex items-center gap-1.5 text-[12px] text-fg-2">
                          <Star className="size-3 fill-warn text-warn" />
                          {r.rating.toFixed(1)} · {r.count} reviews
                        </div>
                      )}
                    </div>
                    <Button
                      size="sm"
                      disabled={save.isPending}
                      onClick={() =>
                        save.mutate({
                          body: {
                            google_place_id: r.place_id,
                            google_place_name: r.name,
                          },
                          ok: `${r.name} is your business on Google`,
                        })
                      }
                    >
                      <Check />
                      This is it
                    </Button>
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}
      </div>
    </Panel>
  );
}

function ReviewSettings({ biz }: { biz: Business }) {
  const save = useSaveBusiness();
  const [show, setShow] = useState(biz.show_reviews);
  const [minRating, setMinRating] = useState(biz.min_rating);
  const [maxReviews, setMaxReviews] = useState(biz.max_reviews);
  const [refresh, setRefresh] = useState(String(biz.refresh_minutes));
  const [override, setOverride] = useState(biz.google_review_url ?? "");
  const minutes = Number(refresh);
  const minutesOk =
    Number.isInteger(minutes) && minutes >= 5 && minutes <= 1440;

  return (
    <Panel>
      <PanelHeader
        title="On your website"
        description="Google's terms let us keep only the place id. Review text is fetched live, held briefly in memory and never saved."
      />
      <form
        className="space-y-5 p-5"
        onSubmit={(e) => {
          e.preventDefault();
          if (!minutesOk) return;
          save.mutate({
            body: {
              show_reviews: show,
              min_rating: minRating,
              max_reviews: maxReviews,
              refresh_minutes: minutes,
              google_review_url: override.trim(),
            },
            ok: "Review settings saved",
          });
        }}
      >
        <div className="flex items-center justify-between gap-4">
          <div>
            <div className="text-[13px] font-medium text-fg">
              Show reviews on the website
            </div>
            <p className="text-[12px] text-fg-3">
              Also controls the reviews widget.
            </p>
          </div>
          <Switch
            checked={show}
            onChange={setShow}
            label="Show reviews on the website"
          />
        </div>
        <div className="grid gap-4 sm:grid-cols-3">
          <div className="space-y-1.5">
            <Label htmlFor="rv-min">Lowest rating shown</Label>
            <select
              id="rv-min"
              className={cn(fieldClass, "block h-11 w-full")}
              value={minRating}
              onChange={(e) => setMinRating(Number(e.target.value))}
            >
              {[1, 2, 3, 4, 5].map((n) => (
                <option key={n} value={n}>
                  {n} star{n > 1 ? "s" : ""} and up
                </option>
              ))}
            </select>
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="rv-max">Reviews shown</Label>
            <select
              id="rv-max"
              className={cn(fieldClass, "block h-11 w-full")}
              value={maxReviews}
              onChange={(e) => setMaxReviews(Number(e.target.value))}
            >
              {[1, 2, 3, 4, 5].map((n) => (
                <option key={n} value={n}>
                  {n}
                </option>
              ))}
            </select>
          </div>
          <Field
            label="Refresh every (minutes)"
            error={minutesOk ? null : "Between 5 and 1440"}
          >
            {(p) => (
              <Input
                {...p}
                type="number"
                inputMode="numeric"
                min={5}
                max={1440}
                value={refresh}
                onChange={(e) => setRefresh(e.target.value)}
              />
            )}
          </Field>
        </div>
        <Field
          label="Review link (optional)"
          hint="Where “Write a review” goes. Leave empty to use Google's link for your listing."
        >
          {(p) => (
            <Input
              {...p}
              placeholder="https://g.page/r/…/review"
              value={override}
              onChange={(e) => setOverride(e.target.value)}
            />
          )}
        </Field>
        <div className="flex justify-end">
          <Button type="submit" disabled={!minutesOk} loading={save.isPending}>
            {!save.isPending && <Check />}
            Save review settings
          </Button>
        </div>
      </form>
    </Panel>
  );
}
