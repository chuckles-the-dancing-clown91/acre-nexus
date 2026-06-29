"use client";

import Link from "next/link";
import {
  ArrowUpRight,
  Banknote,
  Building2,
  DoorOpen,
  Gauge,
  Plus,
  ShieldCheck,
} from "lucide-react";
import { useAuth } from "@/lib/auth";
import { usePortfolioSummary, useProperties } from "@/lib/queries";
import { PageHeader, StatCard, EmptyState } from "@/components/ui/page";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge, statusTone } from "@/components/ui";
import { Button } from "@/components/ui/button";

export default function DashboardPage() {
  const { user } = useAuth();
  const summary = usePortfolioSummary();
  const props = useProperties();

  const firstName = user?.name?.split(" ")[0] ?? "there";

  // Platform staff at the HQ context have no single portfolio.
  if (user?.is_platform_staff && user.active_tenant_id == null) {
    return (
      <div className="space-y-6">
        <PageHeader
          eyebrow="Acre HQ"
          title={`Welcome back, ${firstName}`}
          description="You're in the platform workspace. Manage tenants, staff, and platform health from the Acre Platform console."
        />
        <EmptyState
          icon={ShieldCheck}
          title="Platform workspace"
          description="Switch into a client workspace from the sidebar to see its portfolio, or open the platform console."
          action={
            <Button asChild>
              <Link href="/console/platform">Open platform console</Link>
            </Button>
          }
        />
      </div>
    );
  }

  const s = summary.data;
  const properties = props.data ?? [];

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Portfolio"
        title={`Good to see you, ${firstName}`}
        description="A live snapshot of your managed portfolio."
        actions={
          <Button asChild>
            <Link href="/console/properties/onboard">
              <Plus className="h-4 w-4" />
              Onboard property
            </Link>
          </Button>
        }
      />

      {/* KPIs */}
      <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
        {summary.isLoading || !s ? (
          Array.from({ length: 4 }).map((_, i) => (
            <div key={i} className="skeleton h-[104px] rounded-xl" />
          ))
        ) : (
          <>
            <StatCard
              label="Monthly revenue"
              value={
                s.kpis.find((k) => k.label.toLowerCase().includes("revenue"))
                  ?.value ??
                `$${(s.monthly_revenue_cents / 100).toLocaleString()}`
              }
              sub="Gross scheduled rent"
              icon={Banknote}
              tone="good"
            />
            <StatCard
              label="Properties"
              value={s.properties}
              sub="Across the portfolio"
              icon={Building2}
            />
            <StatCard
              label="Units"
              value={s.units}
              sub={`${s.occupied_units} occupied`}
              icon={DoorOpen}
            />
            <StatCard
              label="Occupancy"
              value={`${s.occupancy_pct}%`}
              sub={`${s.occupied_units} / ${s.units} units`}
              icon={Gauge}
              tone={
                s.occupancy_pct >= 90
                  ? "good"
                  : s.occupancy_pct >= 75
                    ? "warn"
                    : "bad"
              }
            />
          </>
        )}
      </div>

      {/* Properties */}
      <Card>
        <CardHeader>
          <CardTitle>Properties</CardTitle>
          <Button asChild variant="ghost" size="sm">
            <Link href="/console/properties">
              View all
              <ArrowUpRight className="h-4 w-4" />
            </Link>
          </Button>
        </CardHeader>
        <CardContent className="p-0">
          {props.isLoading ? (
            <div className="space-y-2 p-4">
              {Array.from({ length: 4 }).map((_, i) => (
                <div key={i} className="skeleton h-12 rounded-lg" />
              ))}
            </div>
          ) : properties.length === 0 ? (
            <EmptyState
              className="m-4 border-0"
              icon={Building2}
              title="No properties yet"
              description="Onboard your first property to start tracking occupancy, rent, and maintenance."
              action={
                <Button asChild>
                  <Link href="/console/properties/onboard">
                    Onboard property
                  </Link>
                </Button>
              }
            />
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-line text-left text-xs font-semibold uppercase tracking-wide text-ink-3">
                    <th className="px-5 py-2.5 font-semibold">Property</th>
                    <th className="px-5 py-2.5 font-semibold">Occupancy</th>
                    <th className="px-5 py-2.5 text-right font-semibold">
                      Monthly rent
                    </th>
                    <th className="px-5 py-2.5 font-semibold">Status</th>
                    <th className="px-5 py-2.5" />
                  </tr>
                </thead>
                <tbody>
                  {properties.map((p) => (
                    <tr
                      key={p.id}
                      className="group border-b border-line last:border-0 hover:bg-surface-2/50"
                    >
                      <td className="px-5 py-3">
                        <Link
                          href={`/console/properties/${p.id}`}
                          className="block"
                        >
                          <div className="font-medium text-ink">{p.name}</div>
                          <div className="text-xs text-ink-3">
                            {p.address}
                            {p.city ? ` · ${p.city}` : ""}
                          </div>
                        </Link>
                      </td>
                      <td className="px-5 py-3">
                        <div className="flex items-center gap-2">
                          <div className="h-1.5 w-20 overflow-hidden rounded-full bg-surface-2">
                            <div
                              className="h-full rounded-full bg-accent"
                              style={{
                                width: `${
                                  p.units
                                    ? Math.round(
                                        (p.occupied_units / p.units) * 100
                                      )
                                    : 0
                                }%`,
                              }}
                            />
                          </div>
                          <span data-numeric className="text-xs text-ink-2">
                            {p.occupancy}
                          </span>
                        </div>
                      </td>
                      <td
                        data-numeric
                        className="px-5 py-3 text-right font-medium text-ink"
                      >
                        {p.monthly_rent_label}
                      </td>
                      <td className="px-5 py-3">
                        <Badge tone={statusTone(p.status)}>{p.status}</Badge>
                      </td>
                      <td className="px-5 py-3 text-right">
                        <ArrowUpRight className="ml-auto h-4 w-4 text-ink-3 opacity-0 transition group-hover:opacity-100" />
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
