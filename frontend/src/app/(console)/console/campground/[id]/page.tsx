"use client";

// One campground: the front desk for a day, a two-week availability grid,
// booking a stay, and the booking rules (seasons, add-ons, deposit).

import { Suspense, useMemo, useState } from "react";
import Link from "next/link";
import { useParams, useRouter, useSearchParams } from "next/navigation";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowLeft,
  ChevronLeft,
  ChevronRight,
  ExternalLink,
  Plus,
  Trash2,
} from "lucide-react";
import { toast } from "sonner";
import {
  addDays,
  campground,
  span,
  STATUS_WORDS,
  statusTone,
  todayIso,
  type Addon,
  type CampConfig,
  type CampDetail,
  type Stay,
  type StayActionName,
} from "@/lib/campground";
import { useAuth } from "@/lib/auth";
import { usd } from "@/lib/format";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Stat, Tabs } from "@/components/ui/data-table";
import { fieldClass } from "@/components/ui/input";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const TABS = [
  ["desk", "Front desk"],
  ["calendar", "Calendar"],
  ["book", "Book a stay"],
  ["settings", "Settings"],
] as const;
type Tab = (typeof TABS)[number][0];

function errMsg(e: unknown) {
  return e instanceof Error ? e.message : "Something went wrong";
}

export default function CampgroundPage() {
  return (
    <Suspense fallback={<Skeleton className="h-64 rounded-2xl" />}>
      <Camp />
    </Suspense>
  );
}

function Camp() {
  const { id } = useParams<{ id: string }>();
  const params = useSearchParams();
  const router = useRouter();
  const tab = (TABS.find(([k]) => k === params.get("tab"))?.[0] ??
    "desk") as Tab;
  const q = useQuery({
    queryKey: ["campground", id],
    queryFn: () => campground.detail(id),
  });
  const d = q.data;
  return (
    <div className="space-y-6">
      <Link
        href="/console/campground"
        className="inline-flex items-center gap-1.5 text-[13px] text-fg-3 hover:text-fg"
      >
        <ArrowLeft className="size-4" />
        Campgrounds
      </Link>
      <PageHeader
        eyebrow="Campground"
        title={d?.name ?? "Campground"}
        description={
          d
            ? `${d.sites.length} sites · check-in ${d.config.check_in_time}, check-out ${d.config.check_out_time}`
            : undefined
        }
      />
      {d && (!d.published || !d.config.booking_open) && (
        <Panel className="px-4 py-3 text-[13px] text-fg-2">
          Guests can&apos;t book online yet.{" "}
          {!d.published ? "Publish the site map, and " : ""}turn on booking
          under Settings.
        </Panel>
      )}
      {d?.published && d.config.booking_open && (
        <a
          href={`/camp/${d.map_id}`}
          target="_blank"
          rel="noreferrer"
          className="inline-flex items-center gap-1.5 text-[13px] font-medium text-accent hover:underline"
        >
          The guest booking page <ExternalLink className="size-3.5" />
        </a>
      )}
      <Tabs
        tabs={TABS}
        value={tab}
        onChange={(k) =>
          router.replace(`/console/campground/${id}?tab=${k}`, {
            scroll: false,
          })
        }
      />
      {q.isLoading && <Skeleton className="h-64 rounded-2xl" />}
      {d && tab === "desk" && <Desk camp={d} />}
      {d && tab === "calendar" && <Calendar camp={d} />}
      {d && tab === "book" && <Book camp={d} />}
      {d && tab === "settings" && <Settings camp={d} />}
    </div>
  );
}

function useAct(campId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (v: {
      id: string;
      action: StayActionName;
      amount_cents?: number;
    }) =>
      campground.act(
        v.id,
        v.action,
        v.amount_cents ? { amount_cents: v.amount_cents } : {}
      ),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ["campground", campId] });
      void qc.invalidateQueries({ queryKey: ["campground-board", campId] });
      void qc.invalidateQueries({ queryKey: ["campground-grid", campId] });
    },
    onError: (e) => toast.error(errMsg(e)),
  });
}

function Desk({ camp }: { camp: CampDetail }) {
  const { can } = useAuth();
  const write = can("property:write");
  const [day, setDay] = useState(todayIso());
  const q = useQuery({
    queryKey: ["campground-board", camp.map_id, day],
    queryFn: () => campground.board(camp.map_id, day),
  });
  const act = useAct(camp.map_id);
  const b = q.data;
  const row = (s: Stay, actions: [StayActionName, string][]) => (
    <li key={s.id} className="flex flex-wrap items-center gap-3 px-5 py-3">
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-[14px] font-semibold text-fg">
            {s.guest_name}
          </span>
          <Badge>{s.site_name}</Badge>
          <Badge tone={statusTone(s.status)}>{STATUS_WORDS[s.status]}</Badge>
          {s.balance_cents > 0 && (
            <Badge tone="warn">{usd(s.balance_cents)} due</Badge>
          )}
        </div>
        <div className="text-[12px] text-fg-3">
          {s.check_in} to {s.check_out} · {s.nights} night
          {s.nights === 1 ? "" : "s"} · {s.guests} guest
          {s.guests === 1 ? "" : "s"}
          {s.vehicle && ` · ${s.vehicle}`}
          {s.rig_length_ft && ` · ${s.rig_length_ft} ft`}
          {s.phone && ` · ${s.phone}`}
        </div>
      </div>
      {write && (
        <div className="flex flex-wrap gap-2">
          {s.balance_cents > 0 && s.status !== "cancelled" && (
            <Button
              size="sm"
              variant="ghost"
              disabled={act.isPending}
              onClick={() =>
                act.mutate({
                  id: s.id,
                  action: "payment",
                  amount_cents: s.balance_cents,
                })
              }
            >
              Paid {usd(s.balance_cents)}
            </Button>
          )}
          {actions.map(([a, label]) => (
            <Button
              key={a}
              size="sm"
              variant={a === "cancel" ? "ghost" : "secondary"}
              disabled={act.isPending}
              onClick={() => act.mutate({ id: s.id, action: a })}
            >
              {label}
            </Button>
          ))}
        </div>
      )}
    </li>
  );
  const section = (
    title: string,
    list: Stay[] | undefined,
    empty: string,
    actions: [StayActionName, string][]
  ) => (
    <Panel>
      <PanelHeader
        title={title}
        description={list ? `${list.length}` : undefined}
      />
      {list && list.length === 0 ? (
        <p className="px-5 pb-5 text-[13px] text-fg-3">{empty}</p>
      ) : (
        <ul className="divide-y divide-line">
          {list?.map((s) => row(s, actions))}
        </ul>
      )}
    </Panel>
  );
  return (
    <div className="space-y-4">
      <div className="flex items-center gap-2">
        <Button
          size="icon"
          variant="ghost"
          aria-label="Day before"
          onClick={() => setDay(addDays(day, -1))}
        >
          <ChevronLeft />
        </Button>
        <input
          type="date"
          aria-label="Day"
          className={fieldClass}
          value={day}
          onChange={(e) => e.target.value && setDay(e.target.value)}
        />
        <Button
          size="icon"
          variant="ghost"
          aria-label="Day after"
          onClick={() => setDay(addDays(day, 1))}
        >
          <ChevronRight />
        </Button>
        <Button size="sm" variant="ghost" onClick={() => setDay(todayIso())}>
          Today
        </Button>
      </div>
      {b && (
        <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
          <Stat label="In house" value={`${b.occupied} of ${b.sites}`} />
          <Stat label="Arriving" value={b.arriving.length} />
          <Stat label="Leaving" value={b.departing.length} />
          <Stat
            label="Requests"
            value={b.requests.length}
            tone={b.requests.length ? "warn" : undefined}
          />
        </div>
      )}
      {q.isLoading && <Skeleton className="h-48 rounded-2xl" />}
      {b && (
        <div className="grid gap-4 lg:grid-cols-2">
          {section("Requests to confirm", b.requests, "No requests waiting.", [
            ["confirm", "Confirm"],
            ["cancel", "Decline"],
          ])}
          {section("Arriving", b.arriving, "Nobody arriving.", [
            ["check_in", "Check in"],
            ["cancel", "Cancel"],
          ])}
          {section("Leaving", b.departing, "Nobody leaving.", [
            ["check_out", "Check out"],
          ])}
          {section("To clean", b.to_clean, "All sites are clean.", [
            ["cleaned", "Cleaned"],
          ])}
          <div className="lg:col-span-2">
            {section("In house", b.in_house, "Nobody staying.", [
              ["check_out", "Check out"],
            ])}
          </div>
        </div>
      )}
    </div>
  );
}

const DAYS = 14;

function Calendar({ camp }: { camp: CampDetail }) {
  const [from, setFrom] = useState(todayIso());
  const q = useQuery({
    queryKey: ["campground-grid", camp.map_id, from],
    queryFn: () =>
      campground.availability(camp.map_id, from, addDays(from, DAYS)),
  });
  const days = useMemo(
    () => Array.from({ length: DAYS }, (_, i) => addDays(from, i)),
    [from]
  );
  return (
    <Panel className="overflow-hidden">
      <div className="flex items-center gap-2 p-4">
        <Button
          size="icon"
          variant="ghost"
          aria-label="Earlier"
          onClick={() => setFrom(addDays(from, -7))}
        >
          <ChevronLeft />
        </Button>
        <input
          type="date"
          aria-label="From"
          className={fieldClass}
          value={from}
          onChange={(e) => e.target.value && setFrom(e.target.value)}
        />
        <Button
          size="icon"
          variant="ghost"
          aria-label="Later"
          onClick={() => setFrom(addDays(from, 7))}
        >
          <ChevronRight />
        </Button>
      </div>
      {q.isLoading && <Skeleton className="m-4 h-48" />}
      {q.data && (
        <div className="overflow-x-auto">
          <div className="min-w-[860px]">
            <div
              className="grid border-y border-line text-[11px] text-fg-3"
              style={{ gridTemplateColumns: `120px repeat(${DAYS}, 1fr)` }}
            >
              <div className="px-3 py-2">Site</div>
              {days.map((d) => {
                const dt = new Date(`${d}T12:00:00`);
                return (
                  <div
                    key={d}
                    className={cn(
                      "border-l border-line px-1 py-2 text-center",
                      d === todayIso() && "text-accent"
                    )}
                  >
                    {dt.toLocaleDateString(undefined, { weekday: "short" })}
                    <div className="font-mono text-fg-2">{dt.getDate()}</div>
                  </div>
                );
              })}
            </div>
            {q.data.map((r) => (
              <div
                key={r.site.id}
                className="relative grid h-11 border-b border-line/60"
                style={{ gridTemplateColumns: `120px repeat(${DAYS}, 1fr)` }}
              >
                <div className="flex items-center gap-1.5 truncate px-3 text-[13px] font-medium text-fg">
                  {r.site.name}
                  {r.site.closed && <Badge tone="bad">closed</Badge>}
                </div>
                {days.map((d) => (
                  <div key={d} className="border-l border-line/60" />
                ))}
                {r.stays.map((s) => {
                  const sp = span(s, from, DAYS);
                  if (!sp) return null;
                  return (
                    <div
                      key={s.id}
                      title={`${s.guest_name}, ${s.check_in} to ${s.check_out}`}
                      className={cn(
                        "absolute top-1.5 bottom-1.5 truncate rounded-lg px-2 text-[12px] leading-8 font-medium",
                        s.status === "held"
                          ? "bg-warn/20 text-warn"
                          : s.status === "checked_in"
                            ? "bg-good/20 text-good"
                            : "bg-accent/20 text-accent"
                      )}
                      style={{
                        left: `calc(120px + (100% - 120px) * ${sp[0] / DAYS} + 2px)`,
                        width: `calc((100% - 120px) * ${(sp[1] - sp[0]) / DAYS} - 4px)`,
                      }}
                    >
                      {s.guest_name}
                    </div>
                  );
                })}
              </div>
            ))}
          </div>
        </div>
      )}
    </Panel>
  );
}

function Book({ camp }: { camp: CampDetail }) {
  const { can } = useAuth();
  const qc = useQueryClient();
  const [site, setSite] = useState(camp.sites.find((s) => !s.closed)?.id ?? "");
  const [checkIn, setCheckIn] = useState(addDays(todayIso(), 1));
  const [checkOut, setCheckOut] = useState(addDays(todayIso(), 3));
  const [addons, setAddons] = useState<Record<string, number>>({});
  const [guest, setGuest] = useState({
    guest_name: "",
    email: "",
    phone: "",
    guests: "2",
    vehicle: "",
    rig: "",
  });
  const input = {
    site_id: site,
    check_in: checkIn,
    check_out: checkOut,
    addons: Object.entries(addons).filter(([, n]) => n > 0) as [
      string,
      number,
    ][],
  };
  const quote = useQuery({
    queryKey: ["campground-quote", camp.map_id, input],
    queryFn: () => campground.quote(camp.map_id, input),
    enabled: !!site && checkOut > checkIn,
    retry: false,
  });
  const book = useMutation({
    mutationFn: () =>
      campground.book(camp.map_id, {
        ...input,
        guest_name: guest.guest_name,
        email: guest.email || undefined,
        phone: guest.phone || undefined,
        guests: Number(guest.guests) || 1,
        vehicle: guest.vehicle || undefined,
        rig_length_ft: guest.rig ? Number(guest.rig) : undefined,
      }),
    onSuccess: (s) => {
      toast.success(`Booked ${s.site_name} for ${s.guest_name}`);
      setGuest({
        guest_name: "",
        email: "",
        phone: "",
        guests: "2",
        vehicle: "",
        rig: "",
      });
      void qc.invalidateQueries({
        queryKey: ["campground-board", camp.map_id],
      });
      void qc.invalidateQueries({ queryKey: ["campground-grid", camp.map_id] });
    },
    onError: (e) => toast.error(errMsg(e)),
  });
  if (camp.sites.length === 0)
    return (
      <EmptyState
        title="No sites on the map"
        description="Draw sites on the campground's site map first."
      />
    );
  const f = (k: keyof typeof guest, label: string, type = "text") => (
    <label className="text-[12px] text-fg-3">
      {label}
      <input
        type={type}
        className={cn(fieldClass, "mt-1 w-full")}
        value={guest[k]}
        onChange={(e) => setGuest({ ...guest, [k]: e.target.value })}
      />
    </label>
  );
  return (
    <div className="grid gap-4 lg:grid-cols-[1fr_340px]">
      <Panel className="space-y-4 p-5">
        <div className="grid gap-3 sm:grid-cols-3">
          <label className="text-[12px] text-fg-3">
            Site
            <select
              className={cn(fieldClass, "mt-1 w-full")}
              value={site}
              onChange={(e) => setSite(e.target.value)}
            >
              {camp.sites.map((s) => (
                <option key={s.id} value={s.id} disabled={s.closed}>
                  {s.name} {s.site_type ? `(${s.site_type})` : ""}{" "}
                  {s.rate_cents_night
                    ? `· ${usd(s.rate_cents_night)}/night`
                    : ""}
                </option>
              ))}
            </select>
          </label>
          <label className="text-[12px] text-fg-3">
            Check in
            <input
              type="date"
              className={cn(fieldClass, "mt-1 w-full")}
              value={checkIn}
              onChange={(e) => setCheckIn(e.target.value)}
            />
          </label>
          <label className="text-[12px] text-fg-3">
            Check out
            <input
              type="date"
              className={cn(fieldClass, "mt-1 w-full")}
              value={checkOut}
              onChange={(e) => setCheckOut(e.target.value)}
            />
          </label>
        </div>
        <div className="grid gap-3 sm:grid-cols-2">
          {f("guest_name", "Guest name")}
          {f("email", "Email", "email")}
          {f("phone", "Phone", "tel")}
          {f("guests", "Guests", "number")}
          {f("vehicle", "Vehicle or rig")}
          {f("rig", "Rig length (ft)", "number")}
        </div>
        {camp.config.addons.length > 0 && (
          <div className="space-y-2">
            <div className="eyebrow">Add-ons</div>
            {camp.config.addons.map((a) => (
              <label
                key={a.key}
                className="flex items-center gap-3 text-[13px] text-fg-2"
              >
                <input
                  type="number"
                  min={0}
                  max={20}
                  aria-label={a.label}
                  className={cn(fieldClass, "w-20 py-1")}
                  value={addons[a.key] ?? 0}
                  onChange={(e) =>
                    setAddons({ ...addons, [a.key]: Number(e.target.value) })
                  }
                />
                {a.label} · {usd(a.price_cents)} per {a.per}
              </label>
            ))}
          </div>
        )}
      </Panel>
      <Panel className="space-y-3 p-5">
        <div className="eyebrow">Price</div>
        {quote.error && (
          <p className="text-[13px] text-bad">{errMsg(quote.error)}</p>
        )}
        {quote.data && (
          <dl className="space-y-1.5 text-[13px]">
            <div className="flex justify-between">
              <dt className="text-fg-3">
                {quote.data.nights} night{quote.data.nights === 1 ? "" : "s"}
                {quote.data.weeks > 0 && ` (${quote.data.weeks} wk)`}
                {quote.data.months > 0 && ` (${quote.data.months} mo)`}
              </dt>
              <dd className="font-mono">{usd(quote.data.base_cents)}</dd>
            </div>
            {quote.data.season_pct !== 0 && (
              <div className="flex justify-between">
                <dt className="text-fg-3">
                  {quote.data.seasons.join(", ")}{" "}
                  {quote.data.season_pct > 0 ? "+" : ""}
                  {quote.data.season_pct}%
                </dt>
                <dd className="font-mono">
                  {usd(quote.data.stay_cents - quote.data.base_cents)}
                </dd>
              </div>
            )}
            {quote.data.addons.map((a) => (
              <div key={a.key} className="flex justify-between">
                <dt className="text-fg-3">
                  {a.label} × {a.qty}
                </dt>
                <dd className="font-mono">{usd(a.cents)}</dd>
              </div>
            ))}
            <div className="flex justify-between border-t border-line pt-1.5 font-semibold text-fg">
              <dt>Total</dt>
              <dd className="font-mono">{usd(quote.data.total_cents)}</dd>
            </div>
            <div className="flex justify-between text-fg-3">
              <dt>Deposit</dt>
              <dd className="font-mono">{usd(quote.data.deposit_cents)}</dd>
            </div>
          </dl>
        )}
        <Button
          className="w-full"
          disabled={
            !can("property:write") || !quote.data || !guest.guest_name.trim()
          }
          loading={book.isPending}
          onClick={() => book.mutate()}
        >
          Book it
        </Button>
      </Panel>
    </div>
  );
}

function Settings({ camp }: { camp: CampDetail }) {
  const { can } = useAuth();
  const write = can("property:write");
  const qc = useQueryClient();
  const [cfg, setCfg] = useState<CampConfig>(camp.config);
  const [season, setSeason] = useState({
    name: "",
    start_md: "06-01",
    end_md: "08-31",
    adjust_pct: "15",
    min_nights: "2",
  });
  const done = (d: CampDetail) => {
    qc.setQueryData(["campground", camp.map_id], d);
    toast.success("Saved");
  };
  const save = useMutation({
    mutationFn: () => campground.setConfig(camp.map_id, cfg),
    onSuccess: done,
    onError: (e) => toast.error(errMsg(e)),
  });
  const add = useMutation({
    mutationFn: () =>
      campground.addSeason(camp.map_id, {
        name: season.name,
        start_md: season.start_md,
        end_md: season.end_md,
        adjust_pct: Number(season.adjust_pct),
        min_nights: Number(season.min_nights),
      }),
    onSuccess: (d) => {
      done(d);
      setSeason({ ...season, name: "" });
    },
    onError: (e) => toast.error(errMsg(e)),
  });
  const del = useMutation({
    mutationFn: (id: string) => campground.deleteSeason(id),
    onSuccess: () =>
      void qc.invalidateQueries({ queryKey: ["campground", camp.map_id] }),
  });
  const setAddon = (i: number, patch: Partial<Addon>) =>
    setCfg({
      ...cfg,
      addons: cfg.addons.map((a, j) => (j === i ? { ...a, ...patch } : a)),
    });
  return (
    <div className="grid gap-4 lg:grid-cols-2">
      <Panel className="space-y-4 p-5">
        <div className="eyebrow">Booking</div>
        <label className="flex items-center gap-2 text-[14px] text-fg">
          <input
            type="checkbox"
            className="size-4 accent-[var(--accent)]"
            checked={cfg.booking_open}
            disabled={!write}
            onChange={(e) => setCfg({ ...cfg, booking_open: e.target.checked })}
          />
          Guests can book online
        </label>
        <div className="grid grid-cols-2 gap-3">
          {(
            [
              ["deposit_pct", "Deposit (%)", "number"],
              ["max_nights", "Longest stay (nights)", "number"],
              ["check_in_time", "Check-in time", "time"],
              ["check_out_time", "Check-out time", "time"],
            ] as const
          ).map(([k, label, type]) => (
            <label key={k} className="text-[12px] text-fg-3">
              {label}
              <input
                type={type}
                disabled={!write}
                className={cn(fieldClass, "mt-1 w-full")}
                value={cfg[k]}
                onChange={(e) =>
                  setCfg({
                    ...cfg,
                    [k]:
                      type === "number"
                        ? Number(e.target.value)
                        : e.target.value,
                  })
                }
              />
            </label>
          ))}
        </div>
        <label className="block text-[12px] text-fg-3">
          Policies guests see
          <textarea
            rows={3}
            disabled={!write}
            className={cn(fieldClass, "mt-1 w-full")}
            value={cfg.policies ?? ""}
            onChange={(e) =>
              setCfg({ ...cfg, policies: e.target.value || null })
            }
          />
        </label>
        <div className="space-y-2">
          <div className="eyebrow">Add-ons</div>
          {cfg.addons.map((a, i) => (
            <div key={i} className="flex flex-wrap gap-2">
              <input
                aria-label="Name"
                className={cn(fieldClass, "min-w-0 flex-1")}
                value={a.label}
                disabled={!write}
                onChange={(e) =>
                  setAddon(i, {
                    label: e.target.value,
                    key:
                      a.key ||
                      e.target.value.toLowerCase().replace(/[^a-z0-9]+/g, "_"),
                  })
                }
              />
              <input
                aria-label="Price"
                type="number"
                className={cn(fieldClass, "w-24")}
                value={a.price_cents / 100}
                disabled={!write}
                onChange={(e) =>
                  setAddon(i, {
                    price_cents: Math.round(Number(e.target.value) * 100),
                  })
                }
              />
              <select
                aria-label="Per"
                className={fieldClass}
                value={a.per}
                disabled={!write}
                onChange={(e) =>
                  setAddon(i, { per: e.target.value as Addon["per"] })
                }
              >
                <option value="stay">per stay</option>
                <option value="night">per night</option>
              </select>
              <Button
                size="icon"
                variant="ghost"
                aria-label="Remove"
                disabled={!write}
                onClick={() =>
                  setCfg({
                    ...cfg,
                    addons: cfg.addons.filter((_, j) => j !== i),
                  })
                }
              >
                <Trash2 />
              </Button>
            </div>
          ))}
          {write && (
            <Button
              size="sm"
              variant="ghost"
              onClick={() =>
                setCfg({
                  ...cfg,
                  addons: [
                    ...cfg.addons,
                    { key: "", label: "", price_cents: 0, per: "stay" },
                  ],
                })
              }
            >
              <Plus />
              Add-on
            </Button>
          )}
        </div>
        {write && (
          <Button loading={save.isPending} onClick={() => save.mutate()}>
            Save
          </Button>
        )}
      </Panel>
      <Panel className="space-y-4 p-5">
        <div className="eyebrow">Seasons</div>
        <p className="text-[13px] text-fg-3">
          Prices move by a percent during a season, and a season can need a
          minimum stay. Site rates are set on the site map.
        </p>
        <ul className="divide-y divide-line rounded-xl border border-line">
          {camp.seasons.length === 0 && (
            <li className="px-3 py-3 text-[13px] text-fg-3">
              No seasons. Site rates apply all year.
            </li>
          )}
          {camp.seasons.map((s) => (
            <li
              key={s.id}
              className="flex items-center gap-3 px-3 py-2 text-[13px]"
            >
              <span className="flex-1 font-medium text-fg">{s.name}</span>
              <span className="text-fg-3">
                {s.start_md} to {s.end_md} · {s.adjust_pct > 0 ? "+" : ""}
                {s.adjust_pct}% · {s.min_nights}+ nights
              </span>
              {write && (
                <Button
                  size="icon"
                  variant="ghost"
                  aria-label={`Remove ${s.name}`}
                  onClick={() => del.mutate(s.id)}
                >
                  <Trash2 />
                </Button>
              )}
            </li>
          ))}
        </ul>
        {write && (
          <div className="grid grid-cols-2 gap-2 sm:grid-cols-5">
            <input
              aria-label="Season name"
              placeholder="Summer"
              className={cn(fieldClass, "col-span-2 sm:col-span-1")}
              value={season.name}
              onChange={(e) => setSeason({ ...season, name: e.target.value })}
            />
            <input
              aria-label="Starts (MM-DD)"
              placeholder="06-01"
              className={fieldClass}
              value={season.start_md}
              onChange={(e) =>
                setSeason({ ...season, start_md: e.target.value })
              }
            />
            <input
              aria-label="Ends (MM-DD)"
              placeholder="08-31"
              className={fieldClass}
              value={season.end_md}
              onChange={(e) => setSeason({ ...season, end_md: e.target.value })}
            />
            <input
              aria-label="Price change (%)"
              type="number"
              className={fieldClass}
              value={season.adjust_pct}
              onChange={(e) =>
                setSeason({ ...season, adjust_pct: e.target.value })
              }
            />
            <input
              aria-label="Minimum nights"
              type="number"
              className={fieldClass}
              value={season.min_nights}
              onChange={(e) =>
                setSeason({ ...season, min_nights: e.target.value })
              }
            />
            <Button
              size="sm"
              className="col-span-2 sm:col-span-5"
              disabled={!season.name.trim()}
              loading={add.isPending}
              onClick={() => add.mutate()}
            >
              Add season
            </Button>
          </div>
        )}
      </Panel>
    </div>
  );
}
