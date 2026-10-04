"use client";

import { useEffect } from "react";
import Link from "next/link";
import { useQuery } from "@tanstack/react-query";
import { motion } from "motion/react";
import {
  ArrowUpRight,
  Building2,
  CalendarClock,
  ClipboardList,
  Landmark,
  Plus,
  Wrench,
} from "lucide-react";
import { api } from "@/lib/api";
import { useRouter } from "next/navigation";
import { useAuth } from "@/lib/auth";
import { isFieldCrew } from "@/components/shell/nav";
import {
  useFinanceSeries,
  usePortfolioSummary,
  useProperties,
} from "@/lib/queries";
import { useUiStore, type AuraTone } from "@/lib/store";
import { activeWorkspace } from "@/lib/workspaces";
import { bpsLabel, compactUsd } from "@/lib/chart";
import { greeting, usd } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { PortfolioSummary } from "@/lib/types";
import { PortfolioSkyline } from "@/components/dashboard/PortfolioSkyline";
import { Ring, Sparkline, TrendChart } from "@/components/charts";
import {
  useActingTenant,
  useHasTenantScope,
  useReach,
} from "@/components/shell/tenant-scope";
import {
  Badge,
  StatusDot,
  statusTone,
  toneText,
  type Tone,
} from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";

const rise = (i: number) => ({
  initial: { opacity: 0, y: 10 },
  animate: { opacity: 1, y: 0 },
  transition: {
    duration: 0.5,
    delay: 0.04 * i,
    ease: [0.22, 1, 0.36, 1] as const,
  },
});

/** The backdrop glows green when the portfolio is full and calm, amber when urgent work is queued. */
function healthAura(s: PortfolioSummary): AuraTone | null {
  if ((s.urgent_tickets ?? 0) > 0) return "warn";
  if (s.occupancy_pct >= 95) return "good";
  return null;
}

export default function DashboardPage() {
  const { user, can } = useAuth();
  const router = useRouter();
  const scoped = useHasTenantScope();
  // Field roles and owners see only their properties; company-wide money and
  // onboarding aren't theirs.
  const { scoped: propertyScoped } = useReach();
  const setAura = useUiStore((s) => s.setAura);
  const { acting } = useActingTenant();
  const summary = usePortfolioSummary({ enabled: scoped });
  const properties = useProperties({ enabled: scoped });
  const series = useFinanceSeries(12, {
    enabled: scoped && !propertyScoped && can("ledger:read"),
  });
  // Staff viewing a client should see that client's name, not "Vantedge HQ".
  const { data: tenants } = useQuery({
    queryKey: ["platform", "tenants"],
    queryFn: () => api.platformTenants(),
    enabled: !!user?.is_platform_staff && !!acting,
  });

  useEffect(() => {
    if (summary.data) setAura(healthAura(summary.data));
    return () => setAura(null);
  }, [summary.data, setAura]);

  // Field crew start on their day, not a portfolio dashboard.
  const field = isFieldCrew(can);
  useEffect(() => {
    if (user && scoped && field) router.replace("/console/my-day");
  }, [user, scoped, field, router]);

  if (!user) return null;
  if (!scoped) return <StaffTenantPicker />;
  if (field) return null;

  const s = summary.data;
  const firstName = user.name.split(" ")[0];
  const workspace = acting
    ? (tenants?.find((t) => t.slug === acting)?.name ?? acting)
    : activeWorkspace(user)?.name;
  const fin = series.data && series.data.months.length > 0 ? series.data : null;

  return (
    <div className="space-y-6">
      <motion.div {...rise(0)}>
        <PageHeader
          eyebrow={new Date().toLocaleDateString(undefined, {
            weekday: "long",
            month: "long",
            day: "numeric",
          })}
          title={`${greeting()}, ${firstName}`}
          description={
            s ? (
              <>
                {workspace && <span className="text-fg">{workspace}</span>}
                {workspace && " · "}
                {propertyScoped
                  ? `Your ${s.properties === 1 ? "property" : `${s.properties} properties`}`
                  : `${s.properties} properties`}{" "}
                · {s.units} units
              </>
            ) : (
              <Skeleton className="h-5 w-64" />
            )
          }
          actions={
            can("property:write") &&
            !propertyScoped && (
              <Button asChild>
                <Link href="/console/properties/onboard">
                  <Plus />
                  Add property
                </Link>
              </Button>
            )
          }
        />
      </motion.div>

      {summary.error && (
        <Panel className="border-bad/30 p-4 text-[13px] text-bad">
          Couldn&apos;t load the portfolio: {summary.error.message}
        </Panel>
      )}

      <section className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4">
        {s ? (
          <>
            <motion.div {...rise(1)}>
              <Kpi
                label="Monthly rent roll"
                value={usd(s.monthly_revenue_cents)}
                hint={
                  fin
                    ? "Rent collected, last 12 months"
                    : `Across ${s.units} units`
                }
                spark={fin?.rent_collected_cents}
              />
            </motion.div>
            <motion.div {...rise(2)}>
              <Panel className="flex h-full items-center gap-5 p-5">
                <div className="min-w-0 flex-1">
                  <div className="eyebrow">Occupancy</div>
                  <div className="figure mt-2 text-[30px] leading-none font-semibold text-fg">
                    {s.occupancy_pct}%
                  </div>
                  <div className="mt-2 text-xs text-fg-3">
                    {s.occupied_units} of {s.units} units occupied
                  </div>
                </div>
                <Ring
                  value={s.occupancy_pct}
                  tone={
                    s.occupancy_pct >= 95
                      ? "good"
                      : s.occupancy_pct >= 85
                        ? "warn"
                        : "bad"
                  }
                  size={64}
                />
              </Panel>
            </motion.div>
            <motion.div {...rise(3)}>
              {fin ? (
                <Kpi
                  label="Net operating income"
                  value={compactUsd(fin.noi_cents[fin.noi_cents.length - 1])}
                  hint="Last month"
                  spark={fin.noi_cents}
                  tone="info"
                />
              ) : (
                <Kpi
                  label="Units"
                  value={String(s.units)}
                  hint={`${s.units - s.occupied_units} vacant`}
                />
              )}
            </motion.div>
            <motion.div {...rise(4)}>
              {fin ? (
                <Kpi
                  label="Portfolio value"
                  value={compactUsd(
                    fin.portfolio_value_cents[
                      fin.portfolio_value_cents.length - 1
                    ]
                  )}
                  hint={`${s.properties} properties`}
                  spark={fin.portfolio_value_cents}
                  tone="plasma"
                />
              ) : (
                <Kpi
                  label="Properties"
                  value={String(s.properties)}
                  hint={
                    propertyScoped ? "Assigned to you" : "In this workspace"
                  }
                />
              )}
            </motion.div>
          </>
        ) : (
          [0, 1, 2, 3].map((i) => (
            <Skeleton key={i} className="h-[132px] rounded-2xl" />
          ))
        )}
      </section>

      {s && <Attention summary={s} />}

      <motion.div {...rise(6)}>
        {properties.data ? (
          properties.data.length > 0 ? (
            <PortfolioSkyline properties={properties.data} />
          ) : (
            <Panel>
              <EmptyState
                icon={<Building2 />}
                title="No properties yet"
                description="Add your first property to see your portfolio come to life here."
                action={
                  can("property:write") && (
                    <Button asChild>
                      <Link href="/console/properties/onboard">
                        <Plus />
                        Add property
                      </Link>
                    </Button>
                  )
                }
              />
            </Panel>
          )
        ) : (
          <Skeleton className="h-[380px] rounded-2xl" />
        )}
      </motion.div>

      {fin && (
        <section>
          <h2 className="mb-3 text-[15px] font-semibold text-fg">
            Last 12 months
          </h2>
          <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
            <TrendChart
              title="Rent collected"
              months={fin.months}
              values={fin.rent_collected_cents}
              format={compactUsd}
              kind="bar"
            />
            <TrendChart
              title="Net operating income"
              months={fin.months}
              values={fin.noi_cents}
              format={compactUsd}
              kind="bar"
              tone="info"
            />
            <TrendChart
              title="Occupancy"
              months={fin.months}
              values={fin.occupancy_bps}
              format={bpsLabel}
              tone="good"
            />
            <TrendChart
              title="Delinquency"
              months={fin.months}
              values={fin.delinquency_bps}
              format={bpsLabel}
              tone="bad"
              invert
            />
          </div>
        </section>
      )}
    </div>
  );
}

function Kpi({
  label,
  value,
  hint,
  spark,
  tone = "accent",
}: {
  label: string;
  value: string;
  hint?: string;
  spark?: number[];
  tone?: Tone;
}) {
  return (
    <Panel className="flex h-full flex-col p-5">
      <div className="eyebrow">{label}</div>
      <div className="figure mt-2 text-[30px] leading-none font-semibold text-fg">
        {value}
      </div>
      {hint && <div className="mt-2 text-xs text-fg-3">{hint}</div>}
      {spark && spark.length > 1 && (
        <Sparkline values={spark} tone={tone} className="-mx-1 mt-auto pt-3" />
      )}
    </Panel>
  );
}

interface AttentionItem {
  key: string;
  label: string;
  value: number;
  hint: string;
  tone: Tone;
  href: string;
  icon: React.ReactNode;
}

/** What needs someone today. Each tile only appears if the viewer can see that area. */
function Attention({ summary: s }: { summary: PortfolioSummary }) {
  const items: AttentionItem[] = [];
  if (s.open_tickets !== null) {
    const urgent = s.urgent_tickets ?? 0;
    items.push({
      key: "tickets",
      label: "Open work orders",
      value: s.open_tickets,
      hint:
        urgent > 0
          ? `${urgent} urgent`
          : s.open_tickets > 0
            ? "None urgent"
            : "All clear",
      tone: urgent > 0 ? "bad" : s.open_tickets > 0 ? "info" : "good",
      href: "/console/maintenance",
      icon: <Wrench />,
    });
  }
  if (s.delinquent_tenants !== null) {
    const n = s.delinquent_tenants;
    items.push({
      key: "delinquent",
      label: n === 1 ? "Tenant behind on rent" : "Tenants behind on rent",
      value: n,
      hint:
        n > 0 && s.delinquent_balance_label
          ? `${s.delinquent_balance_label} outstanding`
          : "Everyone's current",
      tone: n > 0 ? "bad" : "good",
      href: "/console/reports",
      icon: <Landmark />,
    });
  }
  if (s.pending_applications !== null) {
    const n = s.pending_applications;
    items.push({
      key: "applications",
      label: n === 1 ? "Application to review" : "Applications to review",
      value: n,
      hint: n > 0 ? "Waiting on screening decisions" : "All caught up",
      tone: n > 0 ? "warn" : "good",
      href: "/console/applications",
      icon: <ClipboardList />,
    });
  }
  if (s.upcoming_reminders !== null) {
    const overdue = s.overdue_reminders ?? 0;
    items.push({
      key: "reminders",
      label: "Due in the next 2 weeks",
      value: s.upcoming_reminders,
      hint: overdue > 0 ? `${overdue} overdue` : "Nothing overdue",
      tone: overdue > 0 ? "bad" : s.upcoming_reminders > 0 ? "info" : "good",
      href: "/console/calendar",
      icon: <CalendarClock />,
    });
  }
  if (items.length === 0) return null;

  return (
    <section>
      <div className="mb-3 flex items-center gap-2">
        <h2 className="text-[15px] font-semibold text-fg">Needs attention</h2>
        <StatusDot
          tone={items.some((i) => i.tone === "bad") ? "bad" : "good"}
          live
        />
      </div>
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4">
        {items.map((item, i) => (
          <motion.div key={item.key} {...rise(5 + i * 0.5)}>
            <Link
              href={item.href}
              className="glass group relative flex h-full flex-col rounded-2xl p-4 transition-[border-color,transform,background-color] duration-300 ease-out-soft hover:-translate-y-px hover:border-line-strong hover:bg-fill-2"
            >
              <div className="flex items-start gap-3.5">
                <div
                  className={cn(
                    "flex size-10 shrink-0 items-center justify-center rounded-xl border [&_svg]:size-[18px]",
                    item.tone === "bad" && "border-bad/30 bg-bad/10 text-bad",
                    item.tone === "warn" &&
                      "border-warn/30 bg-warn/10 text-warn",
                    item.tone === "info" &&
                      "border-info/30 bg-info/10 text-info",
                    item.tone === "good" &&
                      "border-good/30 bg-good/10 text-good"
                  )}
                >
                  {item.icon}
                </div>
                <div className="min-w-0 flex-1">
                  <div className="figure text-[26px] leading-none font-semibold text-fg">
                    {item.value}
                  </div>
                  <div className="mt-1.5 text-[13px] text-fg-2">
                    {item.label}
                  </div>
                </div>
                <ArrowUpRight className="size-4 text-fg-4 transition group-hover:translate-x-0.5 group-hover:-translate-y-0.5 group-hover:text-fg-2" />
              </div>
              <div
                className={cn("mt-3 text-xs font-medium", toneText(item.tone))}
              >
                {item.hint}
              </div>
            </Link>
          </motion.div>
        ))}
      </div>
    </section>
  );
}

/** Platform staff pick which client to look at before any client data loads. */
function StaffTenantPicker() {
  const { set } = useActingTenant();
  const { data: tenants, isLoading } = useQuery({
    queryKey: ["platform", "tenants"],
    queryFn: () => api.platformTenants(),
  });

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Vantedge HQ"
        title="Choose a client to view"
        description="You're signed in as platform staff. Pick a workspace to see its console exactly as its team does."
      />
      <Panel className="overflow-hidden">
        <PanelHeader
          title="Client workspaces"
          description={tenants ? `${tenants.length} workspaces` : undefined}
        />
        <ul className="mt-4 divide-y divide-line border-t border-line">
          {isLoading &&
            [0, 1, 2].map((i) => (
              <li key={i} className="px-5 py-4">
                <Skeleton className="h-5 w-1/3" />
              </li>
            ))}
          {tenants?.map((t) => (
            <li key={t.id}>
              <button
                type="button"
                onClick={() => set(t.slug)}
                className="group flex w-full items-center gap-4 px-5 py-4 text-left transition hover:bg-fill-2"
              >
                <span className="flex size-9 shrink-0 items-center justify-center rounded-xl border border-line-strong bg-fill font-display text-sm font-semibold text-fg-2">
                  {t.name.charAt(0)}
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-[14px] font-medium text-fg">
                    {t.name}
                  </span>
                  <span className="block truncate font-mono text-xs text-fg-3">
                    {t.slug}
                  </span>
                </span>
                <span className="hidden text-right text-xs text-fg-3 sm:block">
                  <span className="block text-fg-2">
                    {t.property_count} properties
                  </span>
                  {t.managed_revenue_label}/mo managed
                </span>
                <Badge tone={statusTone(t.status)}>{t.status}</Badge>
                <ArrowUpRight className="size-4 text-fg-4 transition group-hover:text-fg-2" />
              </button>
            </li>
          ))}
        </ul>
      </Panel>
    </div>
  );
}
