"use client";

// One property: its photo and headline numbers, units, open work orders, and
// the people assigned to it. The company decides who is assigned; the people
// assigned see the property but not the controls.

import { useMemo, useState } from "react";
import Link from "next/link";
import { useParams } from "next/navigation";
import { useQuery } from "@tanstack/react-query";
import { motion } from "motion/react";
import {
  ArrowLeft,
  Building2,
  DoorOpen,
  MapPin,
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

const OPEN = new Set(["open", "in_progress", "on_hold", "scheduled", "new"]);

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
  const { id } = useParams<{ id: string }>();
  const { can } = useAuth();
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

      <motion.div {...rise(0)}>
        <Panel className="overflow-hidden">
          <div className="relative h-44 bg-fill sm:h-56">
            {p?.image_url ? (
              // eslint-disable-next-line @next/next/no-img-element -- street photo from our blob store
              <img
                src={p.image_url}
                alt=""
                className="size-full object-cover"
              />
            ) : (
              <div className="size-full bg-[radial-gradient(120%_120%_at_0%_0%,color-mix(in_oklab,var(--accent)_30%,transparent),transparent)]" />
            )}
            <div className="absolute inset-0 bg-gradient-to-t from-black/70 via-black/10 to-transparent" />
            <div className="absolute inset-x-5 bottom-4 text-white">
              {p ? (
                <>
                  <div className="mb-2 flex flex-wrap gap-2">
                    <Badge tone={statusTone(p.status)}>{p.status}</Badge>
                    {p.property_type && <Badge>{p.property_type}</Badge>}
                  </div>
                  <h1 className="text-[26px] leading-tight font-semibold sm:text-[32px]">
                    {p.name}
                  </h1>
                  <div className="mt-1 flex items-center gap-1.5 text-[14px] text-white/80">
                    <MapPin className="size-4" />
                    {[
                      p.address,
                      p.city,
                      [p.state, p.postal_code].filter(Boolean).join(" "),
                    ]
                      .filter(Boolean)
                      .join(", ")}
                  </div>
                </>
              ) : (
                <Skeleton className="h-10 w-72" />
              )}
            </div>
          </div>
        </Panel>
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

      <div className="grid gap-4 xl:grid-cols-[minmax(0,1.6fr)_minmax(0,1fr)]">
        <div className="space-y-4">
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
                      <Badge tone={t.priority === "urgent" ? "bad" : "neutral"}>
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
