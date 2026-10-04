"use client";

// Plan the day: pick a day and a person, let the system lay the jobs out
// (booked visits at their times, the rest nearest first, the supply run up
// front), look over the shopping list by store, and accept the route. Accepting
// books the visits, settles parts against stock, and tells the office what to
// order. The second tab is the shopping list for the week by day and store.

import { Suspense, useEffect, useMemo, useState } from "react";
import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  AlertTriangle,
  ArrowLeft,
  CalendarCheck,
  Car,
  Check,
  ExternalLink,
  MapPin,
  Package,
  Phone,
  Route as RouteIcon,
  ShoppingCart,
  Store,
  Warehouse,
} from "lucide-react";
import { toast } from "sonner";
import { useAuth } from "@/lib/auth";
import {
  dayplan,
  dollars,
  hours,
  stopsToAccept,
  storeSearch,
  type Route,
  type ShopItem,
  type StoreGroup,
} from "@/lib/dayplan";
import { desk } from "@/lib/servicedesk";
import { ymd } from "@/lib/appointments";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const field =
  "rounded-xl border border-line bg-surface px-3 py-2 text-[13px] text-fg outline-none focus:border-accent";

function tomorrow(): string {
  const d = new Date();
  d.setDate(d.getDate() + 1);
  return ymd(d);
}

export default function PlanPage() {
  return (
    <Suspense fallback={<Skeleton className="h-64 rounded-2xl" />}>
      <PlanInner />
    </Suspense>
  );
}

function PlanInner() {
  const { can, user } = useAuth();
  const manage = can("maintenance:manage");
  const params = useSearchParams();
  const qc = useQueryClient();
  const [tab, setTab] = useState<"route" | "shopping">(
    params.get("tab") === "shopping" ? "shopping" : "route"
  );
  const [date, setDate] = useState(params.get("date") || tomorrow());
  const [who, setWho] = useState(params.get("who") || "");
  const [start, setStart] = useState("");
  const techs = useQuery({
    queryKey: ["techs", "all"],
    queryFn: () => desk.techs(),
  });

  const propose = useMutation({
    mutationFn: () =>
      dayplan.propose({
        date,
        assignee_user_id: who || undefined,
        start: start || undefined,
      }),
    onError: (e) =>
      toast.error(e instanceof Error ? e.message : "Couldn't plan it"),
  });
  const route = propose.data;
  // Plan on arrival when the link carried a day and a person.
  useEffect(() => {
    if (params.get("date") && !propose.data && !propose.isPending)
      propose.mutate();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const accept = useMutation({
    mutationFn: (r: Route) =>
      dayplan.accept({
        date: r.date,
        assignee_user_id: who,
        stops: stopsToAccept(r.stops),
      }),
    onSuccess: (a) => {
      toast.success(
        `${a.booked} visit${a.booked === 1 ? "" : "s"} booked for ${a.assignee_name}` +
          (a.to_order.length
            ? `; the office has ${a.to_order.reduce((n, s) => n + s.items.length, 0)} parts to order`
            : "")
      );
      void qc.invalidateQueries({ queryKey: ["tickets"] });
      void qc.invalidateQueries({ queryKey: ["appointments"] });
      void qc.invalidateQueries({ queryKey: ["to-schedule"] });
      propose.mutate();
    },
    onError: (e) =>
      toast.error(e instanceof Error ? e.message : "Couldn't book it"),
  });

  return (
    <div className="space-y-6">
      <Link
        href="/console/maintenance"
        className="inline-flex items-center gap-1.5 text-[13px] text-fg-3 transition hover:text-fg"
      >
        <ArrowLeft className="size-4" />
        Service desk
      </Link>
      <PageHeader
        eyebrow="Service desk"
        title="Plan the day"
        description="The jobs due, in a sensible order with times, and what to pick up on the way."
      />

      <div
        className="flex gap-1 rounded-xl bg-fill p-1 sm:w-fit"
        role="tablist"
      >
        {(
          [
            ["route", "Route", <RouteIcon key="r" className="size-4" />],
            [
              "shopping",
              "Shopping",
              <ShoppingCart key="s" className="size-4" />,
            ],
          ] as const
        ).map(([k, label, icon]) => (
          <button
            key={k}
            role="tab"
            type="button"
            aria-selected={tab === k}
            onClick={() => setTab(k)}
            className={cn(
              "flex flex-1 items-center justify-center gap-1.5 rounded-lg px-4 py-1.5 text-[13px] font-medium transition",
              tab === k
                ? "bg-surface text-fg shadow-sm"
                : "text-fg-3 hover:text-fg"
            )}
          >
            {icon}
            {label}
          </button>
        ))}
      </div>

      {tab === "shopping" && <ShoppingTab />}

      {tab === "route" && (
        <>
          <Panel className="p-4">
            <div className="flex flex-col gap-3 md:flex-row md:items-end">
              <label className="flex flex-col gap-1 text-xs text-fg-3">
                Day
                <input
                  type="date"
                  aria-label="Day"
                  className={field}
                  value={date}
                  onChange={(e) => setDate(e.target.value)}
                />
              </label>
              <label className="flex flex-col gap-1 text-xs text-fg-3">
                Who
                <select
                  aria-label="Who"
                  className={field}
                  value={who}
                  onChange={(e) => setWho(e.target.value)}
                >
                  <option value="">Everyone due that day</option>
                  {user && <option value={user.id}>Me</option>}
                  {techs.data
                    ?.filter((t) => t.user_id !== user?.id)
                    .map((t) => (
                      <option key={t.user_id} value={t.user_id}>
                        {t.name}
                      </option>
                    ))}
                </select>
              </label>
              <label className="flex flex-col gap-1 text-xs text-fg-3">
                Start
                <input
                  type="time"
                  aria-label="Start"
                  className={field}
                  value={start}
                  onChange={(e) => setStart(e.target.value)}
                />
              </label>
              <Button
                onClick={() => propose.mutate()}
                disabled={propose.isPending}
              >
                <RouteIcon />
                {route ? "Plan again" : "Propose a route"}
              </Button>
            </div>
          </Panel>

          {propose.isPending && <Skeleton className="h-64 rounded-2xl" />}

          {route && route.stops.length === 0 && route.unplaced.length === 0 && (
            <Panel>
              <EmptyState
                icon={<CalendarCheck />}
                title="Nothing due that day"
                description="Work orders with that due date or a visit booked that day would show here. Try the To schedule list on the service desk."
              />
            </Panel>
          )}

          {route && (route.stops.length > 0 || route.unplaced.length > 0) && (
            <div className="grid gap-4 xl:grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)]">
              <div className="space-y-4">
                <Panel>
                  <PanelHeader
                    title={`${route.assignee_name ? `${route.assignee_name}'s` : "The"} day, ${new Date(`${route.date}T00:00:00`).toLocaleDateString(undefined, { weekday: "long", month: "short", day: "numeric" })}`}
                    description={`${route.stops.filter((s) => s.kind === "job").length} stops · ${hours(route.total_minutes)} with ${hours(route.drive_minutes)} driving`}
                    action={
                      manage && (
                        <Button
                          onClick={() => {
                            if (!who) {
                              toast.error("Pick who takes the day first.");
                              return;
                            }
                            accept.mutate(route);
                          }}
                          disabled={
                            accept.isPending ||
                            route.stops.every((s) => s.kind !== "job")
                          }
                        >
                          <Check />
                          Accept the route
                        </Button>
                      )
                    }
                  />
                  <ol className="divide-y divide-line">
                    {route.stops.map((s, i) => (
                      <StopRow
                        key={`${s.kind}-${s.ticket_id ?? i}`}
                        s={s}
                        n={i + 1}
                      />
                    ))}
                  </ol>
                </Panel>
                {route.unplaced.length > 0 && (
                  <Panel>
                    <PanelHeader
                      title="Doesn't fit"
                      description="These spill past the end of the day. Move the due date, hand them to someone else, or start earlier."
                    />
                    <ol className="divide-y divide-line">
                      {route.unplaced.map((s, i) => (
                        <StopRow key={s.ticket_id ?? i} s={s} n={null} />
                      ))}
                    </ol>
                  </Panel>
                )}
              </div>
              <div className="space-y-4">
                <Panel>
                  <PanelHeader
                    title="Before leaving"
                    description={
                      route.stores.length
                        ? `Buy at ${route.stores.length} store${route.stores.length === 1 ? "" : "s"}${route.stores.reduce((n, s) => n + s.est_cents, 0) ? `, about ${dollars(route.stores.reduce((n, s) => n + s.est_cents, 0))}` : ""}.`
                        : "Nothing to buy."
                    }
                  />
                  <div className="space-y-3 p-4 pt-0">
                    {route.stores.map((g) => (
                      <StoreList key={g.store} g={g} />
                    ))}
                    {route.from_stock.length > 0 && (
                      <div>
                        <div className="mb-1 flex items-center gap-1.5 text-[13px] font-medium text-fg">
                          <Warehouse className="size-4 text-fg-3" />
                          Pull from stock
                        </div>
                        <ul className="space-y-1 text-[13px] text-fg-2">
                          {route.from_stock.map((i) => (
                            <li
                              key={i.id}
                              className="flex justify-between gap-2"
                            >
                              <span>
                                {i.quantity} × {i.name}
                              </span>
                              <span className="truncate text-fg-3">
                                {i.ticket_title}
                              </span>
                            </li>
                          ))}
                        </ul>
                      </div>
                    )}
                    {route.low_stock.length > 0 && (
                      <LowStockList rows={route.low_stock} />
                    )}
                  </div>
                </Panel>
                <Panel className="p-4 text-[13px] text-fg-3">
                  Accepting books each stop as a confirmed visit with the person
                  on it (the resident hears the time), hands unassigned work to
                  them, settles every work order&apos;s parts against stock with
                  the day before as the need-by, and emails the office what to
                  order and what&apos;s running low. Order from the{" "}
                  <Link
                    href="/console/maintenance"
                    className="text-accent hover:underline"
                  >
                    close-out
                  </Link>{" "}
                  as usual.
                </Panel>
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}

function StopRow({ s, n }: { s: Route["stops"][number]; n: number | null }) {
  const body = (
    <>
      <span
        className={cn(
          "flex size-8 shrink-0 items-center justify-center rounded-full border text-[13px] font-semibold",
          s.kind === "store"
            ? "border-info/40 bg-info/10 text-info"
            : s.fixed
              ? "border-good/40 bg-good/10 text-good"
              : "border-line bg-fill text-fg-2"
        )}
      >
        {s.kind === "store" ? <Store className="size-4" /> : (n ?? "·")}
      </span>
      <span className="min-w-0 flex-1">
        <span className="flex flex-wrap items-center gap-2">
          <span className="text-[14px] font-medium text-fg">{s.title}</span>
          {s.fixed && <Badge tone="good">booked</Badge>}
          {s.priority === "urgent" || s.priority === "high" ? (
            <Badge tone="bad">{s.priority}</Badge>
          ) : null}
          {!s.assignee_user_id && s.kind === "job" && <Badge>unassigned</Badge>}
          {s.note && (
            <Badge tone="warn">
              <AlertTriangle className="size-3" />
              {s.note}
            </Badge>
          )}
        </span>
        <span className="mt-0.5 block text-[13px] text-fg-2">
          <span className="figure font-medium text-fg">{s.when_words}</span>
          {" · "}
          {hours(s.minutes)} on site
          {s.drive_minutes > 0 && (
            <>
              {" · "}
              <Car className="inline size-3.5 align-[-2px]" /> {s.drive_minutes}
              m
            </>
          )}
          {s.tasks_total > 0 && ` · ${s.tasks_done}/${s.tasks_total} tasks`}
          {s.to_buy > 0 && ` · ${s.to_buy} to buy`}
          {s.from_stock > 0 && ` · ${s.from_stock} from stock`}
        </span>
        {s.kind === "job" && (
          <span className="mt-0.5 flex flex-wrap items-center gap-x-3 gap-y-0.5 text-xs text-fg-3">
            <span className="flex items-center gap-1">
              <MapPin className="size-3" />
              {s.property_name}
              {s.address ? `, ${s.address}` : ""}
            </span>
            {s.with_name && (
              <span className="flex items-center gap-1">
                <Phone className="size-3" />
                {s.with_name}
                {s.with_phone ? ` ${s.with_phone}` : ""}
              </span>
            )}
          </span>
        )}
      </span>
    </>
  );
  return (
    <li>
      {s.ticket_id ? (
        <Link
          href={`/console/maintenance/${s.ticket_id}`}
          className="flex items-start gap-3 px-4 py-3 transition hover:bg-fill"
        >
          {body}
        </Link>
      ) : (
        <div className="flex items-start gap-3 px-4 py-3">{body}</div>
      )}
    </li>
  );
}

function StoreList({ g }: { g: StoreGroup }) {
  return (
    <div>
      <div className="mb-1 flex items-center justify-between text-[13px] font-medium text-fg">
        <span className="flex items-center gap-1.5">
          <Store className="size-4 text-fg-3" />
          {g.store}
          <span className="text-fg-3">· {g.items.length}</span>
        </span>
        {g.est_cents > 0 && (
          <span className="figure text-fg-3">{dollars(g.est_cents)}</span>
        )}
      </div>
      <ul className="space-y-1 text-[13px] text-fg-2">
        {g.items.map((i) => (
          <ShopRow key={i.id} i={i} store={g.store} />
        ))}
      </ul>
    </div>
  );
}

function ShopRow({ i, store }: { i: ShopItem; store: string }) {
  const href =
    i.url ?? storeSearch(store, i.name) ?? storeSearch("Home Depot", i.name);
  return (
    <li className="flex items-center justify-between gap-2">
      <span className="min-w-0">
        <span>
          {i.quantity} × {i.name}
        </span>
        <span className="block truncate text-xs text-fg-3">
          {i.ticket_title} · {i.property_name}
          {i.status === "pick_up" ? " · to pick up" : ""}
        </span>
      </span>
      <span className="flex shrink-0 items-center gap-2">
        {i.unit_cost_cents ? (
          <span className="figure text-xs text-fg-3">
            {dollars(i.unit_cost_cents * i.quantity)}
          </span>
        ) : null}
        {href && (
          <a
            href={href}
            target="_blank"
            rel="noreferrer"
            aria-label={`Buy ${i.name}`}
            className="text-fg-3 hover:text-accent"
          >
            <ExternalLink className="size-3.5" />
          </a>
        )}
      </span>
    </li>
  );
}

function LowStockList({ rows }: { rows: Route["low_stock"] }) {
  return (
    <div className="rounded-xl border border-warn/30 bg-warn/[0.06] p-3">
      <div className="mb-1 flex items-center gap-1.5 text-[13px] font-medium text-warn">
        <Package className="size-4" />
        Running low after this
      </div>
      <ul className="space-y-0.5 text-[13px] text-fg-2">
        {rows.map((l) => (
          <li key={l.inventory_item_id}>
            {l.name}: {l.on_hand} on hand, {l.after} after · reorder at{" "}
            {l.reorder_level}
          </li>
        ))}
      </ul>
    </div>
  );
}

function ShoppingTab() {
  const [from, setFrom] = useState(ymd(new Date()));
  const [days, setDays] = useState(7);
  const to = useMemo(() => {
    const d = new Date(`${from}T00:00:00`);
    d.setDate(d.getDate() + days - 1);
    return ymd(d);
  }, [from, days]);
  const q = useQuery({
    queryKey: ["shopping", from, to],
    queryFn: () => dayplan.shopping(from, to),
  });
  return (
    <div className="space-y-4">
      <Panel className="flex flex-col gap-3 p-4 md:flex-row md:items-end">
        <label className="flex flex-col gap-1 text-xs text-fg-3">
          From
          <input
            type="date"
            aria-label="From"
            className={field}
            value={from}
            onChange={(e) => setFrom(e.target.value)}
          />
        </label>
        <div className="flex gap-1 rounded-xl bg-fill p-1">
          {[1, 3, 7, 14].map((n) => (
            <button
              key={n}
              type="button"
              onClick={() => setDays(n)}
              className={cn(
                "rounded-lg px-3 py-1.5 text-xs font-medium transition",
                days === n
                  ? "bg-surface text-fg shadow-sm"
                  : "text-fg-3 hover:text-fg"
              )}
            >
              {n === 1 ? "Day" : `${n} days`}
            </button>
          ))}
        </div>
        {q.data && q.data.total_cents > 0 && (
          <span className="text-[13px] text-fg-3 md:ml-auto">
            About{" "}
            <span className="figure font-medium text-fg">
              {dollars(q.data.total_cents)}
            </span>{" "}
            to buy
          </span>
        )}
      </Panel>
      {q.isLoading && <Skeleton className="h-48 rounded-2xl" />}
      {q.data && q.data.days.length === 0 && q.data.undated.length === 0 && (
        <Panel>
          <EmptyState
            icon={<ShoppingCart />}
            title="Nothing to buy"
            description="Parts on the work orders due in this window show here by day and store."
          />
        </Panel>
      )}
      {q.data?.low_stock.length ? (
        <LowStockList rows={q.data.low_stock} />
      ) : null}
      {q.data?.days.map((d) => (
        <Panel key={d.day}>
          <PanelHeader
            title={new Date(`${d.day}T00:00:00`).toLocaleDateString(undefined, {
              weekday: "long",
              month: "short",
              day: "numeric",
            })}
            description={`${d.stores.reduce((n, s) => n + s.items.length, 0)} to buy${d.from_stock.length ? `, ${d.from_stock.length} from stock` : ""}`}
          />
          <div className="grid gap-4 p-4 pt-0 md:grid-cols-2">
            {d.stores.map((g) => (
              <StoreList key={g.store} g={g} />
            ))}
            {d.from_stock.length > 0 && (
              <div>
                <div className="mb-1 flex items-center gap-1.5 text-[13px] font-medium text-fg">
                  <Warehouse className="size-4 text-fg-3" />
                  Pull from stock
                </div>
                <ul className="space-y-1 text-[13px] text-fg-2">
                  {d.from_stock.map((i) => (
                    <li key={i.id} className="flex justify-between gap-2">
                      <span>
                        {i.quantity} × {i.name}
                      </span>
                      <span className="truncate text-fg-3">
                        {i.ticket_title}
                      </span>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        </Panel>
      ))}
      {q.data && q.data.undated.length > 0 && (
        <Panel>
          <PanelHeader
            title="No date yet"
            description="Parts on open work orders that haven't been scheduled. Plan their day and they move up."
          />
          <div className="grid gap-4 p-4 pt-0 md:grid-cols-2">
            {q.data.undated.map((g) => (
              <StoreList key={g.store} g={g} />
            ))}
          </div>
        </Panel>
      )}
    </div>
  );
}
