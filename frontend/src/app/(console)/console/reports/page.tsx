"use client";

// Reports: how operations and leasing are going, and the standard PM reports
// (rent roll, T-12, aging, delinquency, owner statement, 1099), each with
// CSV and PDF export.

import { Suspense, useEffect, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import Link from "next/link";
import { useQuery } from "@tanstack/react-query";
import { Download, TriangleAlert } from "lucide-react";
import { api } from "@/lib/api";
import { analytics, hours } from "@/lib/analytics";
import { useAuth } from "@/lib/auth";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { TrendChart } from "@/components/charts";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DataTable, Stat, Tabs } from "@/components/ui/data-table";
import { fieldClass } from "@/components/ui/input";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { usd } from "@/lib/format";

type Key =
  | "operations"
  | "leasing"
  | "rent-roll"
  | "t12"
  | "aging"
  | "delinquency"
  | "owner-statement"
  | "1099";

const TABS = [
  ["operations", "Operations"],
  ["leasing", "Leasing"],
  ["rent-roll", "Rent roll"],
  ["t12", "T-12"],
  ["aging", "Aging"],
  ["delinquency", "Delinquency"],
  ["owner-statement", "Owner statement"],
  ["1099", "1099"],
] as const;

function asKey(t: string | null): Key {
  return (TABS.find(([k]) => k === t)?.[0] as Key) ?? "operations";
}

async function download(path: string, filename: string) {
  const blob = await api.downloadReport(path);
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
}

function Exports({ base, name }: { base: string; name: string }) {
  const join = base.includes("?") ? "&" : "?";
  return (
    <div className="flex gap-2">
      {(["csv", "pdf"] as const).map((f) => (
        <Button
          key={f}
          size="sm"
          variant="secondary"
          onClick={() => download(`${base}${join}format=${f}`, `${name}.${f}`)}
        >
          <Download />
          {f.toUpperCase()}
        </Button>
      ))}
    </div>
  );
}

export default function ReportsPage() {
  return (
    <Suspense fallback={<Skeleton className="h-64 rounded-2xl" />}>
      <Reports />
    </Suspense>
  );
}

function Reports() {
  const params = useSearchParams();
  const router = useRouter();
  const tab = asKey(params.get("tab"));
  const choose = (k: Key) =>
    router.replace(`/console/reports?tab=${k}`, { scroll: false });
  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Reports"
        title="Reports"
        description="How operations and leasing are going, and the reports owners and accountants ask for."
      />
      <Tabs tabs={TABS} value={tab} onChange={choose} />
      {tab === "operations" && <OperationsTab />}
      {tab === "leasing" && <LeasingTab />}
      {tab === "rent-roll" && <RentRoll />}
      {tab === "t12" && <T12 />}
      {tab === "aging" && <Aging />}
      {tab === "delinquency" && <Delinquency />}
      {tab === "owner-statement" && <OwnerStatement />}
      {tab === "1099" && <Tax1099 />}
    </div>
  );
}

function useReady() {
  const { can } = useAuth();
  return useHasTenantScope() && can("report:read");
}

function OperationsTab() {
  const ready = useReady();
  const [months, setMonths] = useState(12);
  const [property, setProperty] = useState("");
  const q = useQuery({
    queryKey: ["analytics", "operations", months, property],
    queryFn: () => analytics.operations(months, property || undefined),
    enabled: ready,
  });
  const d = q.data;
  const labels = d?.months.map((m) => m.month) ?? [];
  return (
    <div className="space-y-6">
      <div className="flex flex-wrap gap-3">
        <select
          aria-label="Period"
          className={fieldClass}
          value={months}
          onChange={(e) => setMonths(Number(e.target.value))}
        >
          {[3, 6, 12, 24].map((m) => (
            <option key={m} value={m}>
              Last {m} months
            </option>
          ))}
        </select>
        <select
          aria-label="Property"
          className={fieldClass}
          value={property}
          onChange={(e) => setProperty(e.target.value)}
        >
          <option value="">All properties</option>
          {(d?.properties ?? []).map((p) => (
            <option key={p.property_id} value={p.property_id}>
              {p.name}
            </option>
          ))}
        </select>
      </div>
      {q.isLoading && <Skeleton className="h-64 rounded-2xl" />}
      {d && (
        <>
          <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
            <Stat
              label="Days to turn"
              value={d.totals.turns.avg_days ?? "—"}
              hint={`${d.totals.turns.count} finished · ${d.turns_open} open`}
            />
            <Stat
              label="Cost to turn"
              value={
                d.totals.turns.avg_cost_cents !== null
                  ? usd(d.totals.turns.avg_cost_cents)
                  : "—"
              }
              hint={`${usd(d.totals.turns.total_cost_cents)} in all`}
            />
            <Stat
              label="Past target"
              value={d.totals.tickets.past_sla}
              tone={d.totals.tickets.past_sla > 0 ? "warn" : "good"}
              hint={`of ${d.totals.tickets.opened} work orders · ${d.tickets_open} open now`}
            />
            <Stat
              label="Rating"
              value={
                d.totals.tickets.avg_rating !== null
                  ? `${d.totals.tickets.avg_rating} / 5`
                  : "—"
              }
              hint={`Resolved in ${hours(d.totals.tickets.avg_hours_to_resolve)} on average`}
            />
          </div>
          <div className="grid gap-4 lg:grid-cols-3">
            <TrendChart
              title="Work orders opened"
              months={labels}
              values={d.months.map((m) => m.tickets.opened)}
              format={(v) => String(v)}
              kind="bar"
            />
            <TrendChart
              title="Past target"
              months={labels}
              values={d.months.map((m) => m.tickets.past_sla)}
              format={(v) => String(v)}
              kind="bar"
              invert
              tone="bad"
            />
            <TrendChart
              title="Days to turn"
              months={labels}
              values={d.months.map((m) => m.turns.avg_days ?? 0)}
              format={(v) => `${v} d`}
              invert
            />
          </div>

          <Panel>
            <PanelHeader
              title="Replace instead of repair"
              description={`Appliances whose repairs in this period passed ${d.replace_share_pct}% of what they cost.`}
            />
            <DataTable
              columns={[
                "Appliance",
                "Property",
                { label: "Repairs", num: true },
                { label: "Spent", num: true },
                { label: "Price", num: true },
                { label: "Share", num: true },
                "",
              ]}
              rows={d.appliances.slice(0, 12).map((a) => [
                a.name,
                a.property_name,
                a.repairs,
                usd(a.spend_cents),
                a.price_cents !== null ? usd(a.price_cents) : "—",
                a.share_pct !== null ? `${a.share_pct}%` : "—",
                a.replace ? (
                  <Badge key="r" tone="bad">
                    Replace
                  </Badge>
                ) : null,
              ])}
              empty="No appliance has had repairs in this period."
            />
          </Panel>

          <div className="grid gap-6 lg:grid-cols-2">
            <Panel>
              <PanelHeader
                title="Keeps coming back"
                description="Three or more work orders of one kind at one property."
              />
              <DataTable
                columns={[
                  "Property",
                  "Kind",
                  { label: "Times", num: true },
                  { label: "Spent", num: true },
                  "Last",
                ]}
                rows={d.repeats.map((r) => [
                  <Link
                    key="p"
                    href={`/console/properties/${r.property_id}`}
                    className="hover:underline"
                  >
                    {r.property_name}
                  </Link>,
                  r.category.replace(/_/g, " "),
                  r.count,
                  usd(r.spend_cents),
                  r.last_opened,
                ])}
                empty="Nothing repeats yet."
              />
            </Panel>
            <Panel>
              <PanelHeader
                title="By kind"
                description="Work orders opened in the period."
              />
              <DataTable
                columns={[
                  "Kind",
                  { label: "Work orders", num: true },
                  { label: "Spent", num: true },
                ]}
                rows={d.categories.map((c) => [
                  c.category.replace(/_/g, " "),
                  c.count,
                  usd(c.spend_cents),
                ])}
              />
            </Panel>
          </div>

          <Panel>
            <PanelHeader title="By property" />
            <DataTable
              columns={[
                "Property",
                { label: "Occupied", num: true },
                { label: "Turns", num: true },
                { label: "Days to turn", num: true },
                { label: "Cost to turn", num: true },
                { label: "Work orders", num: true },
                { label: "Past target", num: true },
                { label: "Rating", num: true },
              ]}
              rows={d.properties.map((p) => [
                <Link
                  key="p"
                  href={`/console/properties/${p.property_id}`}
                  className="hover:underline"
                >
                  {p.name}
                </Link>,
                `${p.occupied}/${p.units}`,
                p.turns.count,
                p.turns.avg_days ?? "—",
                p.turns.avg_cost_cents !== null
                  ? usd(p.turns.avg_cost_cents)
                  : "—",
                p.tickets.opened,
                p.tickets.past_sla,
                p.tickets.avg_rating ?? "—",
              ])}
            />
          </Panel>
        </>
      )}
    </div>
  );
}

function LeasingTab() {
  const ready = useReady();
  const [months, setMonths] = useState(6);
  const q = useQuery({
    queryKey: ["analytics", "leasing", months],
    queryFn: () => analytics.leasing(months),
    enabled: ready,
  });
  const d = q.data;
  return (
    <div className="space-y-6">
      <select
        aria-label="Period"
        className={fieldClass}
        value={months}
        onChange={(e) => setMonths(Number(e.target.value))}
      >
        {[3, 6, 12].map((m) => (
          <option key={m} value={m}>
            Last {m} months
          </option>
        ))}
      </select>
      {q.isLoading && <Skeleton className="h-48 rounded-2xl" />}
      {d && (
        <>
          <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
            <Stat label="Tour requests" value={d.tours} />
            <Stat
              label="Applications"
              value={d.applications}
              hint={
                d.tour_to_application_pct !== null
                  ? `${d.tour_to_application_pct}% of tours`
                  : undefined
              }
            />
            <Stat
              label="Leases signed"
              value={d.leases}
              hint={
                d.application_to_lease_pct !== null
                  ? `${d.application_to_lease_pct}% of applications`
                  : undefined
              }
            />
            <Stat
              label="Days on market"
              value={d.avg_days_on_market ?? "—"}
              hint="For listings that leased"
            />
          </div>
          <Panel>
            <PanelHeader
              title="Listings"
              description="Longest on the market first."
            />
            <DataTable
              columns={[
                "Listing",
                "Status",
                { label: "Rent", num: true },
                "Listed",
                { label: "Days", num: true },
                { label: "Tours", num: true },
                { label: "Applications", num: true },
              ]}
              rows={d.listings.map((l) => [
                l.title,
                l.leased ? (
                  <Badge key="s" tone="good">
                    Leased
                  </Badge>
                ) : (
                  <Badge key="s">{l.status}</Badge>
                ),
                usd(l.rent_cents),
                l.listed_on,
                l.days_on_market,
                l.tours,
                l.applications,
              ])}
              empty="No listings yet."
            />
          </Panel>
        </>
      )}
    </div>
  );
}

function ReportPanel({
  title,
  summary,
  tools,
  children,
}: {
  title: string;
  summary?: React.ReactNode;
  tools?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <Panel>
      <PanelHeader title={title} description={summary} action={tools} />
      {children}
    </Panel>
  );
}

function RentRoll() {
  const ready = useReady();
  const q = useQuery({
    queryKey: ["report", "rent-roll"],
    queryFn: () => api.rentRoll(),
    enabled: ready,
  });
  if (!q.data) return <Skeleton className="h-64 rounded-2xl" />;
  const d = q.data;
  return (
    <ReportPanel
      title="Rent roll"
      summary={`${d.lease_count} leases · ${d.total_rent_label} a month · ${d.total_balance_label} owed`}
      tools={<Exports base="/reports/rent-roll/export" name="rent-roll" />}
    >
      <DataTable
        columns={[
          "Property",
          "Unit",
          "Resident",
          { label: "Rent", num: true },
          "Term",
          "Status",
          "Payment",
          { label: "Balance", num: true },
        ]}
        rows={d.rows.map((r) => [
          r.property_name,
          r.unit,
          r.tenant_name,
          r.rent_label,
          r.term,
          r.status,
          r.payment_status,
          r.balance_label,
        ])}
        totals={[
          "Total",
          "",
          "",
          d.total_rent_label,
          "",
          "",
          "",
          d.total_balance_label,
        ]}
      />
    </ReportPanel>
  );
}

function useEntities() {
  const ready = useReady();
  return useQuery({
    queryKey: ["llcs"],
    queryFn: api.legalEntities,
    enabled: ready,
  });
}

function EntityPicker({
  value,
  onChange,
}: {
  value: string;
  onChange: (v: string) => void;
}) {
  const es = useEntities();
  useEffect(() => {
    if (!value && es.data?.[0]) onChange(es.data[0].id);
  }, [es.data, value, onChange]);
  return (
    <select
      aria-label="Entity"
      className={fieldClass}
      value={value}
      onChange={(e) => onChange(e.target.value)}
    >
      {(es.data ?? []).map((e) => (
        <option key={e.id} value={e.id}>
          {e.name}
        </option>
      ))}
    </select>
  );
}

function T12() {
  const [entity, setEntity] = useState("");
  const q = useQuery({
    queryKey: ["report", "t12", entity],
    queryFn: () => api.t12Report(entity),
    enabled: !!entity,
  });
  const es = useEntities();
  if (es.data && es.data.length === 0)
    return (
      <EmptyState
        title="No entities yet"
        description="Add an LLC to see its income statement."
      />
    );
  const d = q.data;
  return (
    <ReportPanel
      title="Trailing twelve months"
      summary={d ? `Net operating income ${d.net_label}` : undefined}
      tools={
        <div className="flex flex-wrap gap-2">
          <EntityPicker value={entity} onChange={setEntity} />
          {entity && (
            <Exports base={`/reports/t12/export?entity=${entity}`} name="t12" />
          )}
        </div>
      }
    >
      {!d ? (
        <Skeleton className="m-5 h-48 rounded-xl" />
      ) : (
        <DataTable
          columns={[
            "Account",
            ...d.months.map((m) => ({ label: m, num: true })),
            { label: "Total", num: true },
          ]}
          rows={[
            ...d.income.map((r) => [
              r.account_name,
              ...r.monthly_cents.map(usd),
              r.total_label,
            ]),
            [
              "Total income",
              ...d.income_totals_cents.map(usd),
              d.total_income_label,
            ],
            ...d.expenses.map((r) => [
              r.account_name,
              ...r.monthly_cents.map(usd),
              r.total_label,
            ]),
            [
              "Total expense",
              ...d.expense_totals_cents.map(usd),
              d.total_expense_label,
            ],
          ]}
          totals={[
            "Net operating income",
            ...d.noi_totals_cents.map(usd),
            d.net_label,
          ]}
        />
      )}
    </ReportPanel>
  );
}

function Aging() {
  const ready = useReady();
  const q = useQuery({
    queryKey: ["report", "aging"],
    queryFn: api.agingReport,
    enabled: ready,
  });
  if (!q.data) return <Skeleton className="h-64 rounded-2xl" />;
  const d = q.data;
  return (
    <ReportPanel
      title="Receivables aging"
      summary={`As of ${d.generated_at}`}
      tools={<Exports base="/reports/aging/export" name="aging" />}
    >
      <DataTable
        columns={[
          "Resident",
          "Property",
          ...["Current", "1–30", "31–60", "61–90", "90+", "Total"].map((l) => ({
            label: l,
            num: true,
          })),
        ]}
        rows={d.rows.map((r) => [
          r.tenant_name,
          r.property_name,
          usd(r.current_cents),
          usd(r.d1_30_cents),
          usd(r.d31_60_cents),
          usd(r.d61_90_cents),
          usd(r.over90_cents),
          usd(r.total_cents),
        ])}
        totals={[
          "Total",
          "",
          usd(d.current_cents),
          usd(d.d1_30_cents),
          usd(d.d31_60_cents),
          usd(d.d61_90_cents),
          usd(d.over90_cents),
          usd(d.total_cents),
        ]}
        empty="Nobody owes anything."
      />
    </ReportPanel>
  );
}

function Delinquency() {
  const ready = useReady();
  const q = useQuery({
    queryKey: ["report", "delinquency"],
    queryFn: api.delinquencyReport,
    enabled: ready,
  });
  if (!q.data) return <Skeleton className="h-64 rounded-2xl" />;
  const d = q.data;
  return (
    <ReportPanel
      title="Delinquency"
      summary={`${d.tenant_count} behind · ${d.total_balance_label} owed`}
      tools={<Exports base="/reports/delinquency/export" name="delinquency" />}
    >
      <DataTable
        columns={[
          "Resident",
          "Property",
          "Unit",
          "Status",
          { label: "Balance", num: true },
          { label: "Days late", num: true },
          "Oldest due",
        ]}
        rows={d.rows.map((r) => [
          r.tenant_name,
          r.property_name,
          r.unit,
          r.payment_status,
          r.balance_label,
          r.days_late,
          r.oldest_due_date ?? "—",
        ])}
        totals={["Total", "", "", "", d.total_balance_label, "", ""]}
        empty="Nobody is behind."
      />
    </ReportPanel>
  );
}

function OwnerStatement() {
  const [entity, setEntity] = useState("");
  const q = useQuery({
    queryKey: ["report", "owner", entity],
    queryFn: () => api.ownerStatement(entity),
    enabled: !!entity,
  });
  const d = q.data;
  return (
    <ReportPanel
      title="Owner statement"
      summary={d ? `${d.period_start} to ${d.period_end}` : undefined}
      tools={
        <div className="flex flex-wrap gap-2">
          <EntityPicker value={entity} onChange={setEntity} />
          {entity && (
            <Exports
              base={`/reports/owner-statement/export?entity=${entity}`}
              name="owner-statement"
            />
          )}
        </div>
      }
    >
      {!d ? (
        <Skeleton className="m-5 h-40 rounded-xl" />
      ) : (
        <DataTable
          columns={["Item", { label: "Amount", num: true }]}
          rows={[
            ["Rent collected", d.rent_collected_label],
            ...d.expense_lines.map((l) => [l.name, `−${l.amount_label}`]),
            ["Operating expenses", `−${d.expenses_label}`],
            ["Management fee", `−${d.mgmt_fee_label}`],
          ]}
          totals={["Net to the owner", d.net_label]}
        />
      )}
    </ReportPanel>
  );
}

function Tax1099() {
  const ready = useReady();
  const now = new Date().getFullYear();
  const [year, setYear] = useState(String(now - 1));
  const q = useQuery({
    queryKey: ["report", "1099", year],
    queryFn: () => api.tax1099(year),
    enabled: ready,
  });
  const d = q.data;
  return (
    <ReportPanel
      title="1099 tax"
      summary={d ? `Paid ${d.threshold_label} or more in ${d.year}` : undefined}
      tools={
        <div className="flex flex-wrap gap-2">
          <select
            aria-label="Year"
            className={fieldClass}
            value={year}
            onChange={(e) => setYear(e.target.value)}
          >
            {[now, now - 1, now - 2].map((y) => (
              <option key={y} value={y}>
                {y}
              </option>
            ))}
          </select>
          <Exports
            base={`/reports/1099/export?year=${year}`}
            name={`1099-${year}`}
          />
        </div>
      }
    >
      {!d ? (
        <Skeleton className="m-5 h-40 rounded-xl" />
      ) : (
        <>
          {d.missing_tin_count > 0 && (
            <p className="mx-5 mb-3 flex items-center gap-2 rounded-xl bg-warn/10 px-3 py-2 text-[13px] text-warn">
              <TriangleAlert className="size-4" />
              {d.missing_tin_count} recipient
              {d.missing_tin_count === 1 ? "" : "s"} missing a W-9. Ask for it
              before filing.
            </p>
          )}
          <DataTable
            columns={[
              "Form",
              "Recipient",
              "TIN",
              "Box",
              { label: "Amount", num: true },
            ]}
            rows={[...d.nec, ...d.misc].map((x) => [
              x.form,
              x.name,
              x.tin ?? (x.missing_tin ? "W-9 missing" : "—"),
              x.box_label,
              x.amount_label,
            ])}
            totals={[
              "Total",
              "",
              "",
              "",
              `${d.nec_total_label} NEC · ${d.misc_total_label} MISC`,
            ]}
            empty="Nobody passed the threshold this year."
          />
        </>
      )}
    </ReportPanel>
  );
}
