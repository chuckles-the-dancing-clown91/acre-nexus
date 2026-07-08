"use client";

// Standard PM reports (roadmap Phase 8): rent roll, T-12, aging, delinquency,
// owner statements, and the annual 1099 tax export — each viewable inline and
// exportable to CSV / PDF.

import { useCallback, useEffect, useState } from "react";
import {
  api,
  type AgingResp,
  type DelinquencyResp,
  type LegalEntity,
  type OwnerStatementResp,
  type RentRollResp,
  type T12Resp,
  type Tax1099Resp,
} from "@/lib/api";
import { Button, Card } from "@/components/ui";
import { logError } from "@/lib/log";

type ReportKey =
  | "rent-roll"
  | "t12"
  | "aging"
  | "delinquency"
  | "owner-statement"
  | "1099";

const TABS: { key: ReportKey; label: string }[] = [
  { key: "rent-roll", label: "Rent roll" },
  { key: "t12", label: "T-12" },
  { key: "aging", label: "Aging" },
  { key: "delinquency", label: "Delinquency" },
  { key: "owner-statement", label: "Owner statement" },
  { key: "1099", label: "1099 tax" },
];

async function download(path: string, filename: string) {
  try {
    const blob = await api.downloadReport(path);
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
  } catch (e) {
    logError("failed to export report", e);
  }
}

function ExportButtons({ base }: { base: string }) {
  return (
    <div className="flex gap-2">
      <Button
        variant="outline"
        onClick={() =>
          download(
            `${base}${base.includes("?") ? "&" : "?"}format=csv`,
            "report.csv"
          )
        }
      >
        CSV
      </Button>
      <Button
        variant="outline"
        onClick={() =>
          download(
            `${base}${base.includes("?") ? "&" : "?"}format=pdf`,
            "report.pdf"
          )
        }
      >
        PDF
      </Button>
    </div>
  );
}

function DataTable({
  headers,
  rows,
  totals,
}: {
  headers: string[];
  rows: string[][];
  totals?: string[];
}) {
  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-line text-left text-xs uppercase tracking-wide text-ink-3">
            {headers.map((h) => (
              <th key={h} className="whitespace-nowrap px-3 py-2 font-semibold">
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.length === 0 ? (
            <tr>
              <td
                colSpan={headers.length}
                className="px-3 py-6 text-center text-ink-3"
              >
                No data.
              </td>
            </tr>
          ) : (
            rows.map((row, i) => (
              <tr key={i} className="border-b border-line/60">
                {row.map((cell, j) => (
                  <td
                    key={j}
                    className="whitespace-nowrap px-3 py-2 tabular-nums"
                  >
                    {cell}
                  </td>
                ))}
              </tr>
            ))
          )}
        </tbody>
        {totals && (
          <tfoot>
            <tr className="border-t-2 border-line font-bold">
              {totals.map((cell, j) => (
                <td
                  key={j}
                  className="whitespace-nowrap px-3 py-2 tabular-nums"
                >
                  {cell}
                </td>
              ))}
            </tr>
          </tfoot>
        )}
      </table>
    </div>
  );
}

function RentRoll() {
  const [data, setData] = useState<RentRollResp | null>(null);
  useEffect(() => {
    api
      .rentRoll()
      .then(setData)
      .catch((e) => logError("rent roll", e));
  }, []);
  if (!data) return <div className="text-ink-3">Loading…</div>;
  return (
    <Card className="space-y-3 p-4">
      <div className="flex items-center justify-between">
        <div className="text-sm text-ink-3">
          {data.lease_count} leases · rent {data.total_rent_label}/mo · balance{" "}
          {data.total_balance_label}
        </div>
        <ExportButtons base="/reports/rent-roll/export" />
      </div>
      <DataTable
        headers={[
          "Property",
          "Unit",
          "Tenant",
          "Rent",
          "Term",
          "Status",
          "Payment",
          "Balance",
        ]}
        rows={data.rows.map((r) => [
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
          "TOTAL",
          "",
          "",
          data.total_rent_label,
          "",
          "",
          "",
          data.total_balance_label,
        ]}
      />
    </Card>
  );
}

function T12() {
  const [entities, setEntities] = useState<LegalEntity[]>([]);
  const [entity, setEntity] = useState<string>("");
  const [data, setData] = useState<T12Resp | null>(null);

  useEffect(() => {
    api
      .legalEntities()
      .then((es) => {
        setEntities(es);
        if (es[0]) setEntity(es[0].id);
      })
      .catch((e) => logError("legal entities", e));
  }, []);

  const load = useCallback(() => {
    if (!entity) return;
    setData(null);
    api
      .t12Report(entity)
      .then(setData)
      .catch((e) => logError("t12", e));
  }, [entity]);

  useEffect(() => {
    load();
  }, [load]);

  const money = (c: number) =>
    (c / 100).toLocaleString(undefined, {
      style: "currency",
      currency: "USD",
      maximumFractionDigits: 0,
    });

  return (
    <Card className="space-y-3 p-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <select
          value={entity}
          onChange={(e) => setEntity(e.target.value)}
          className="rounded-lg border border-line bg-surface-2 px-3 py-2 text-sm"
        >
          {entities.map((e) => (
            <option key={e.id} value={e.id}>
              {e.name}
            </option>
          ))}
        </select>
        {entity && (
          <ExportButtons base={`/reports/t12/export?entity=${entity}`} />
        )}
      </div>
      {!data ? (
        <div className="text-ink-3">Loading…</div>
      ) : (
        <DataTable
          headers={["Account", ...data.months, "Total"]}
          rows={[
            ...data.income.map((r) => [
              r.account_name,
              ...r.monthly_cents.map(money),
              r.total_label,
            ]),
            [
              "Total income",
              ...data.income_totals_cents.map(money),
              data.total_income_label,
            ],
            ...data.expenses.map((r) => [
              r.account_name,
              ...r.monthly_cents.map(money),
              r.total_label,
            ]),
            [
              "Total expense",
              ...data.expense_totals_cents.map(money),
              data.total_expense_label,
            ],
          ]}
          totals={["NOI", ...data.noi_totals_cents.map(money), data.net_label]}
        />
      )}
    </Card>
  );
}

function Aging() {
  const [data, setData] = useState<AgingResp | null>(null);
  useEffect(() => {
    api
      .agingReport()
      .then(setData)
      .catch((e) => logError("aging", e));
  }, []);
  if (!data) return <div className="text-ink-3">Loading…</div>;
  const money = (c: number) =>
    (c / 100).toLocaleString(undefined, {
      style: "currency",
      currency: "USD",
      maximumFractionDigits: 0,
    });
  return (
    <Card className="space-y-3 p-4">
      <div className="flex items-center justify-between">
        <div className="text-sm text-ink-3">As of {data.generated_at}</div>
        <ExportButtons base="/reports/aging/export" />
      </div>
      <DataTable
        headers={[
          "Tenant",
          "Property",
          "Current",
          "1–30",
          "31–60",
          "61–90",
          "90+",
          "Total",
        ]}
        rows={data.rows.map((r) => [
          r.tenant_name,
          r.property_name,
          money(r.current_cents),
          money(r.d1_30_cents),
          money(r.d31_60_cents),
          money(r.d61_90_cents),
          money(r.over90_cents),
          money(r.total_cents),
        ])}
        totals={[
          "TOTAL",
          "",
          money(data.current_cents),
          money(data.d1_30_cents),
          money(data.d31_60_cents),
          money(data.d61_90_cents),
          money(data.over90_cents),
          money(data.total_cents),
        ]}
      />
    </Card>
  );
}

function Delinquency() {
  const [data, setData] = useState<DelinquencyResp | null>(null);
  useEffect(() => {
    api
      .delinquencyReport()
      .then(setData)
      .catch((e) => logError("delinquency", e));
  }, []);
  if (!data) return <div className="text-ink-3">Loading…</div>;
  return (
    <Card className="space-y-3 p-4">
      <div className="flex items-center justify-between">
        <div className="text-sm text-ink-3">
          {data.tenant_count} tenants behind · {data.total_balance_label} owed
        </div>
        <ExportButtons base="/reports/delinquency/export" />
      </div>
      <DataTable
        headers={[
          "Tenant",
          "Property",
          "Unit",
          "Status",
          "Balance",
          "Days late",
          "Oldest due",
        ]}
        rows={data.rows.map((r) => [
          r.tenant_name,
          r.property_name,
          r.unit,
          r.payment_status,
          r.balance_label,
          String(r.days_late),
          r.oldest_due_date ?? "—",
        ])}
        totals={["TOTAL", "", "", "", data.total_balance_label, "", ""]}
      />
    </Card>
  );
}

function OwnerStatement() {
  const [entities, setEntities] = useState<LegalEntity[]>([]);
  const [entity, setEntity] = useState<string>("");
  const [data, setData] = useState<OwnerStatementResp | null>(null);

  useEffect(() => {
    api
      .legalEntities()
      .then((es) => {
        setEntities(es);
        if (es[0]) setEntity(es[0].id);
      })
      .catch((e) => logError("legal entities", e));
  }, []);

  useEffect(() => {
    if (!entity) return;
    setData(null);
    api
      .ownerStatement(entity)
      .then(setData)
      .catch((e) => logError("owner statement", e));
  }, [entity]);

  return (
    <Card className="space-y-3 p-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <select
          value={entity}
          onChange={(e) => setEntity(e.target.value)}
          className="rounded-lg border border-line bg-surface-2 px-3 py-2 text-sm"
        >
          {entities.map((e) => (
            <option key={e.id} value={e.id}>
              {e.name}
            </option>
          ))}
        </select>
        {entity && (
          <ExportButtons
            base={`/reports/owner-statement/export?entity=${entity}`}
          />
        )}
      </div>
      {!data ? (
        <div className="text-ink-3">Loading…</div>
      ) : (
        <>
          <div className="text-sm text-ink-3">
            {data.period_start} → {data.period_end}
          </div>
          <DataTable
            headers={["Item", "Amount"]}
            rows={[
              ["Rent collected", data.rent_collected_label],
              ...data.expense_lines.map((l) => [
                `  ${l.name}`,
                `-${l.amount_label}`,
              ]),
              ["Total operating expenses", `-${data.expenses_label}`],
              ["Management fee", `-${data.mgmt_fee_label}`],
            ]}
            totals={["NET OWNER DRAW", data.net_label]}
          />
        </>
      )}
    </Card>
  );
}

function Tax1099() {
  const currentYear = new Date().getFullYear();
  const years = [currentYear - 1, currentYear, currentYear - 2];
  const [year, setYear] = useState<string>(String(currentYear - 1));
  const [data, setData] = useState<Tax1099Resp | null>(null);

  useEffect(() => {
    setData(null);
    api
      .tax1099(year)
      .then(setData)
      .catch((e) => logError("1099", e));
  }, [year]);

  const recipientRows = (r: Tax1099Resp) =>
    [...r.nec, ...r.misc].map((x) => [
      x.form,
      x.name,
      x.tin ?? "—",
      x.box_label,
      x.amount_label,
    ]);

  return (
    <Card className="space-y-3 p-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <select
          value={year}
          onChange={(e) => setYear(e.target.value)}
          className="rounded-lg border border-line bg-surface-2 px-3 py-2 text-sm"
        >
          {years.map((y) => (
            <option key={y} value={y}>
              {y}
            </option>
          ))}
        </select>
        <ExportButtons base={`/reports/1099/export?year=${year}`} />
      </div>
      {!data ? (
        <div className="text-ink-3">Loading…</div>
      ) : (
        <>
          <div className="text-sm text-ink-3">
            Recipients at or above {data.threshold_label} · NEC{" "}
            {data.nec_total_label} · MISC {data.misc_total_label}
          </div>
          <DataTable
            headers={["Form", "Recipient", "TIN/EIN", "Box", "Amount"]}
            rows={recipientRows(data)}
            totals={[
              "TOTAL",
              "",
              "",
              "",
              data.nec_total_cents + data.misc_total_cents === 0
                ? "$0"
                : `$${(
                    (data.nec_total_cents + data.misc_total_cents) /
                    100
                  ).toLocaleString(undefined, { maximumFractionDigits: 0 })}`,
            ]}
          />
        </>
      )}
    </Card>
  );
}

export default function ReportsPage() {
  const [tab, setTab] = useState<ReportKey>("rent-roll");
  return (
    <div className="space-y-5">
      <header>
        <h1 className="font-display text-2xl font-bold">Reports</h1>
        <p className="mt-1 text-sm text-ink-3">
          Standard PM reports off the live ledger + rentals. Export any report
          to CSV or PDF.
        </p>
      </header>

      <div className="flex flex-wrap gap-1 border-b border-line">
        {TABS.map((t) => (
          <button
            key={t.key}
            onClick={() => setTab(t.key)}
            className={
              tab === t.key
                ? "border-b-2 border-accent px-4 py-2 text-sm font-bold text-ink"
                : "px-4 py-2 text-sm font-semibold text-ink-3 hover:text-ink"
            }
          >
            {t.label}
          </button>
        ))}
      </div>

      {tab === "rent-roll" && <RentRoll />}
      {tab === "t12" && <T12 />}
      {tab === "aging" && <Aging />}
      {tab === "delinquency" && <Delinquency />}
      {tab === "owner-statement" && <OwnerStatement />}
      {tab === "1099" && <Tax1099 />}
    </div>
  );
}
