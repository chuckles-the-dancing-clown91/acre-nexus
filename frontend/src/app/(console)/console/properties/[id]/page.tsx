"use client";

// One property, in full: its photo and headline numbers, then tabs for what
// needs doing (to-dos, units, open work orders, people), the parcel and the
// money around it, the equipment, permits and plans, schools, and insurance.
// The company decides who is assigned; the people assigned see the property
// but not the controls.

import { Readiness } from "@/components/property/Readiness";
import { Suspense, useMemo, useState } from "react";
import Link from "next/link";
import { useParams, useRouter, useSearchParams } from "next/navigation";
import { useQuery } from "@tanstack/react-query";
import { motion } from "motion/react";
import {
  ArrowLeft,
  Building2,
  DoorOpen,
  UserPlus,
  Users,
  Wrench,
  X,
} from "lucide-react";
import { api, ApiError } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { usd } from "@/lib/format";
import { occupancyPct } from "@/lib/properties";
import {
  queryKeys,
  useAssignments,
  useCreateAssignment,
  useDeleteAssignment,
  useMembers,
} from "@/lib/queries";
import { ASSIGNABLE_RELATIONSHIPS } from "@/lib/types";
import { useReach } from "@/components/shell/tenant-scope";
import { Ring } from "@/components/charts";
import { Badge, statusTone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { ActionItems } from "@/components/property/ActionItems";
import { Facts } from "@/components/property/Facts";
import { Gallery } from "@/components/property/Gallery";
import { History } from "@/components/property/History";
import { Summary } from "@/components/property/Summary";
import { Insurance } from "@/components/property/Insurance";
import { Parcel } from "@/components/property/Parcel";
import { Permits } from "@/components/property/Permits";
import { Mandates } from "@/components/property/Mandates";
import { Plans } from "@/components/property/Plans";
import { Safety } from "@/components/property/Safety";
import { Area, Schools } from "@/components/property/Schools";
import { Systems } from "@/components/property/Systems";
import { cn } from "@/lib/utils";

const TABS = [
  { key: "overview", label: "Overview" },
  { key: "parcel", label: "Parcel and money" },
  { key: "systems", label: "Appliances and systems" },
  { key: "permits", label: "Permits and plans" },
  { key: "history", label: "History" },
  { key: "schools", label: "Schools and area" },
  { key: "insurance", label: "Insurance" },
] as const;

const OPEN = new Set(["open", "triage", "scheduled", "in_progress", "on_hold"]);

const rise = (i: number) => ({
  initial: { opacity: 0, y: 10 },
  animate: { opacity: 1, y: 0 },
  transition: {
    duration: 0.45,
    delay: i * 0.04,
    ease: [0.22, 1, 0.36, 1] as const,
  },
});

export default function PropertyPage() {
  return (
    <Suspense fallback={<Skeleton className="h-64" />}>
      <PropertyView />
    </Suspense>
  );
}

function PropertyView() {
  const { id } = useParams<{ id: string }>();
  const { can } = useAuth();
  const params = useSearchParams();
  const router = useRouter();
  const tab = TABS.some((t) => t.key === params.get("tab"))
    ? (params.get("tab") as (typeof TABS)[number]["key"])
    : "overview";
  const write = can("property:write");
  const intel = useQuery({
    queryKey: ["intel", id],
    queryFn: () => api.propertyIntel(id),
    enabled: tab === "insurance",
  });
  const property = useQuery({
    queryKey: queryKeys.property(id),
    queryFn: () => api.property(id),
    retry: (n, e) => !(e instanceof ApiError && e.status < 500) && n < 2,
  });
  const units = useQuery({
    queryKey: ["properties", id, "units"],
    queryFn: () => api.units(id),
    enabled: property.isSuccess && can("lease:read"),
  });
  const tickets = useQuery({
    queryKey: ["properties", id, "tickets"],
    queryFn: () => api.propertyTickets(id),
    enabled: property.isSuccess && can("maintenance:read"),
  });

  if (property.error) {
    const missing =
      property.error instanceof ApiError && property.error.status === 404;
    return (
      <Panel className="mx-auto mt-10 max-w-lg">
        <EmptyState
          icon={<Building2 />}
          title={
            missing
              ? "This property isn't in your view"
              : "Couldn't load the property"
          }
          description={
            missing
              ? "It may belong to another company, or you aren't assigned to it."
              : property.error.message
          }
          action={
            <Button variant="secondary" asChild>
              <Link href="/console/properties">
                <ArrowLeft />
                All properties
              </Link>
            </Button>
          }
        />
      </Panel>
    );
  }

  const p = property.data;
  const occ = p ? occupancyPct(p.units, p.occupied_units) : 0;
  const open = (tickets.data ?? []).filter((t) => OPEN.has(t.status));

  return (
    <div className="space-y-6">
      <Link
        href="/console/properties"
        className="inline-flex items-center gap-1.5 text-[13px] text-fg-3 transition hover:text-fg"
      >
        <ArrowLeft className="size-4" />
        Properties
      </Link>

      <motion.div
        {...rise(0)}
        className="grid gap-4 xl:grid-cols-[minmax(0,1.6fr)_minmax(0,1fr)]"
      >
        <Gallery
          propertyId={id}
          name={p?.name ?? "Property"}
          fallbackUrl={p?.image_url ?? null}
          manage={write}
        />
        <Summary property={p} propertyId={id} />
      </motion.div>

      <section className="grid grid-cols-2 gap-3 xl:grid-cols-4">
        <motion.div {...rise(1)}>
          <Panel className="flex h-full items-center gap-4 p-5">
            <div className="min-w-0 flex-1">
              <div className="eyebrow">Occupancy</div>
              <div className="figure mt-2 text-[28px] leading-none font-semibold text-fg">
                {p ? `${occ}%` : "—"}
              </div>
              <div className="mt-2 text-xs text-fg-3">
                {p ? `${p.occupied_units} of ${p.units} units` : " "}
              </div>
            </div>
            <Ring
              value={occ}
              size={54}
              tone={occ >= 95 ? "good" : occ >= 85 ? "warn" : "bad"}
            />
          </Panel>
        </motion.div>
        <motion.div {...rise(2)}>
          <Stat
            label="Rent roll"
            value={p ? usd(p.monthly_rent_cents) : "—"}
            hint="per month"
          />
        </motion.div>
        <motion.div {...rise(3)}>
          <Stat
            label="Open work orders"
            value={tickets.data ? String(open.length) : "—"}
            hint={
              open.some((t) => t.priority === "urgent")
                ? "includes urgent"
                : "none urgent"
            }
          />
        </motion.div>
        <motion.div {...rise(4)}>
          <Stat
            label="Manager"
            value={p?.manager || "Unassigned"}
            hint={p?.year_built ? `Built ${p.year_built}` : undefined}
            small
          />
        </motion.div>
      </section>

      <nav
        className="-mx-1 flex gap-1 overflow-x-auto border-b border-line px-1"
        aria-label="Property sections"
      >
        {TABS.map((t) => (
          <button
            key={t.key}
            type="button"
            aria-current={tab === t.key ? "page" : undefined}
            onClick={() =>
              router.replace(t.key === "overview" ? `?` : `?tab=${t.key}`, {
                scroll: false,
              })
            }
            className={cn(
              "-mb-px shrink-0 border-b-2 px-3 py-2.5 text-[13px] font-medium whitespace-nowrap transition",
              tab === t.key
                ? "border-accent text-fg"
                : "border-transparent text-fg-3 hover:text-fg"
            )}
          >
            {t.label}
          </button>
        ))}
      </nav>

      {tab === "parcel" && <Parcel propertyId={id} />}
      {tab === "systems" && (
        <div className="space-y-4">
          <Systems propertyId={id} canOrder={can("maintenance:manage")} />
          <Mandates propertyId={id} manage={can("maintenance:manage")} />
        </div>
      )}
      {tab === "permits" && (
        <div className="space-y-4">
          <Permits propertyId={id} manage={write} />
          <Plans propertyId={id} manage={write} />
        </div>
      )}
      {tab === "history" && <History propertyId={id} />}
      {tab === "schools" && (
        <>
          <Area propertyId={id} />
          <Safety propertyId={id} />
          <Schools propertyId={id} manage={write} />
        </>
      )}
      {tab === "insurance" && (
        <Insurance
          propertyId={id}
          manage={write}
          floodZone={intel.data?.detail?.flood_zone}
        />
      )}

      {tab === "overview" && (
        <div className="grid gap-4 xl:grid-cols-[minmax(0,1.6fr)_minmax(0,1fr)]">
          <div className="space-y-4">
            <Readiness propertyId={id} manage={write} />
            <ActionItems propertyId={id} manage={write} />
            <Facts property={p} propertyId={id} manage={write} />
            {can("lease:read") && (
              <Panel>
                <PanelHeader
                  title="Units"
                  description={
                    units.data ? `${units.data.length} on record` : undefined
                  }
                />
                <div className="p-2 pt-3">
                  {units.data?.length === 0 && (
                    <EmptyState
                      icon={<DoorOpen />}
                      title="No units on record"
                      className="py-8"
                    />
                  )}
                  {units.data && units.data.length > 0 && (
                    <div className="overflow-x-auto">
                      <table className="w-full text-[13px]">
                        <thead>
                          <tr className="text-left text-[11px] tracking-wide text-fg-3 uppercase">
                            <th className="px-3 py-2 font-medium">Unit</th>
                            <th className="px-3 py-2 font-medium">Layout</th>
                            <th className="px-3 py-2 text-right font-medium">
                              Market rent
                            </th>
                            <th className="px-3 py-2 text-right font-medium">
                              Status
                            </th>
                          </tr>
                        </thead>
                        <tbody>
                          {units.data.map((u) => (
                            <tr
                              key={u.id}
                              className="border-t border-line text-fg-2"
                            >
                              <td className="px-3 py-2.5 font-medium text-fg">
                                {u.unit_number}
                              </td>
                              <td className="px-3 py-2.5">
                                {[
                                  u.beds != null && `${u.beds} bd`,
                                  u.baths != null && `${u.baths} ba`,
                                  u.sqft != null && `${u.sqft} sqft`,
                                ]
                                  .filter(Boolean)
                                  .join(" · ") || "—"}
                              </td>
                              <td className="figure px-3 py-2.5 text-right">
                                {u.market_rent_label ?? "—"}
                              </td>
                              <td className="px-3 py-2.5 text-right">
                                <Badge tone={statusTone(u.status)}>
                                  {u.status}
                                </Badge>
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  )}
                  {units.isLoading && <Skeleton className="m-3 h-24" />}
                </div>
              </Panel>
            )}

            {can("maintenance:read") && (
              <Panel>
                <PanelHeader title="Open work orders" />
                <div className="p-2 pt-3">
                  {tickets.data && open.length === 0 && (
                    <EmptyState
                      icon={<Wrench />}
                      title="Nothing open"
                      description="Every work order here is closed."
                      className="py-8"
                    />
                  )}
                  <ul className="divide-y divide-line">
                    {open.map((t) => (
                      <li
                        key={t.id}
                        className="flex items-center gap-3 px-3 py-2.5"
                      >
                        <div className="min-w-0 flex-1">
                          <div className="truncate text-[13px] font-medium text-fg">
                            {t.title}
                          </div>
                          <div className="text-xs text-fg-3">
                            {t.category}
                            {t.location ? ` · ${t.location}` : ""}
                          </div>
                        </div>
                        <Badge
                          tone={t.priority === "urgent" ? "bad" : "neutral"}
                        >
                          {t.priority}
                        </Badge>
                        <Badge tone={statusTone(t.status)}>
                          {t.status.replace("_", " ")}
                        </Badge>
                      </li>
                    ))}
                  </ul>
                  {tickets.isLoading && <Skeleton className="m-3 h-16" />}
                </div>
              </Panel>
            )}
          </div>

          <People propertyId={id} />
        </div>
      )}
    </div>
  );
}

function Stat({
  label,
  value,
  hint,
  small,
}: {
  label: string;
  value: string;
  hint?: string;
  small?: boolean;
}) {
  return (
    <Panel className="flex h-full flex-col p-5">
      <div className="eyebrow">{label}</div>
      <div
        className={
          small
            ? "mt-2 truncate text-[17px] font-semibold text-fg"
            : "figure mt-2 text-[28px] leading-none font-semibold text-fg"
        }
      >
        {value}
      </div>
      {hint && <div className="mt-2 text-xs text-fg-3">{hint}</div>}
    </Panel>
  );
}

/** Who is assigned here. Only the company can change it. */
function People({ propertyId }: { propertyId: string }) {
  const { can } = useAuth();
  const { scoped } = useReach();
  const manage = !scoped && can("property:write");
  const assignments = useAssignments("property", propertyId);
  const members = useMembers({ enabled: manage && can("member:read") });
  const create = useCreateAssignment("property", propertyId);
  const remove = useDeleteAssignment("property", propertyId);
  const [adding, setAdding] = useState(false);
  const [userId, setUserId] = useState("");
  const [relationship, setRelationship] = useState("property_manager");

  const candidates = useMemo(
    () =>
      (members.data ?? []).filter(
        (m) => m.status === "active" && m.profile_type !== "renter"
      ),
    [members.data]
  );

  return (
    <Panel className="h-fit">
      <PanelHeader
        title="People"
        description="Assigned people see this property and nothing else of the company's, unless their role covers the whole company."
        action={
          manage &&
          !adding && (
            <Button
              size="sm"
              variant="secondary"
              onClick={() => setAdding(true)}
            >
              <UserPlus />
              Assign
            </Button>
          )
        }
      />
      <div className="space-y-3 p-5 pt-4">
        {adding && (
          <form
            className="space-y-2 rounded-xl border border-line bg-fill/40 p-3"
            onSubmit={(e) => {
              e.preventDefault();
              if (!userId) return;
              create.mutate(
                { user_id: userId, relationship },
                {
                  onSuccess: () => {
                    setAdding(false);
                    setUserId("");
                  },
                }
              );
            }}
          >
            <label className="block text-xs text-fg-3">
              Person
              <select
                value={userId}
                onChange={(e) => setUserId(e.target.value)}
                className="mt-1 w-full rounded-lg border border-line bg-surface px-2.5 py-2 text-[13px] text-fg"
                required
              >
                <option value="">Choose a teammate…</option>
                {candidates.map((m) => (
                  <option key={m.user_id} value={m.user_id}>
                    {m.name} · {m.profile_type.replace(/_/g, " ")}
                  </option>
                ))}
              </select>
            </label>
            <label className="block text-xs text-fg-3">
              As
              <select
                value={relationship}
                onChange={(e) => setRelationship(e.target.value)}
                className="mt-1 w-full rounded-lg border border-line bg-surface px-2.5 py-2 text-[13px] text-fg"
              >
                {ASSIGNABLE_RELATIONSHIPS.map((r) => (
                  <option key={r.key} value={r.key}>
                    {r.label}
                  </option>
                ))}
              </select>
            </label>
            <div className="flex justify-end gap-2 pt-1">
              <Button
                type="button"
                size="sm"
                variant="ghost"
                onClick={() => setAdding(false)}
              >
                Cancel
              </Button>
              <Button
                type="submit"
                size="sm"
                disabled={!userId || create.isPending}
              >
                Assign
              </Button>
            </div>
          </form>
        )}

        {assignments.data?.length === 0 && !adding && (
          <EmptyState
            icon={<Users />}
            title="Nobody assigned"
            description={
              manage
                ? "Assign a property manager, leasing agent or maintenance tech."
                : undefined
            }
            className="py-6"
          />
        )}
        <ul className="space-y-2">
          {assignments.data?.map((a) => (
            <li
              key={a.id}
              className="flex items-center gap-3 rounded-xl border border-line px-3 py-2.5"
            >
              <span className="flex size-8 shrink-0 items-center justify-center rounded-full bg-accent/15 text-[12px] font-semibold text-accent">
                {a.user_name
                  .split(" ")
                  .map((w) => w[0])
                  .slice(0, 2)
                  .join("")}
              </span>
              <div className="min-w-0 flex-1">
                <div className="truncate text-[13px] font-medium text-fg">
                  {a.user_name}
                </div>
                <div className="truncate text-xs text-fg-3">
                  {a.relationship_label}
                  {a.is_primary ? " · lead" : ""}
                </div>
              </div>
              {manage && (
                <button
                  type="button"
                  aria-label={`Unassign ${a.user_name}`}
                  disabled={remove.isPending}
                  onClick={() => {
                    if (confirm(`Unassign ${a.user_name} from this property?`))
                      remove.mutate(a.id);
                  }}
                  className="rounded-lg p-1.5 text-fg-3 transition hover:bg-fill-2 hover:text-bad"
                >
                  <X className="size-4" />
                </button>
              )}
            </li>
          ))}
        </ul>
        {assignments.isLoading && <Skeleton className="h-14" />}
      </div>
    </Panel>
  );
}
