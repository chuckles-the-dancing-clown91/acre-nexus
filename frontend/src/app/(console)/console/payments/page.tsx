"use client";

// Payments: rent, deposits and fees across the workspace, as each moves from
// due to processing to paid or failed (or late). Settled payments post to the
// ledger and issue a receipt on their own.

import { useMemo, useState } from "react";
import Link from "next/link";
import { useQuery } from "@tanstack/react-query";
import { Lock } from "lucide-react";
import { api } from "@/lib/api";
import { queryKeys } from "@/lib/queries";
import { usd } from "@/lib/format";
import { useReady } from "@/lib/money-extra";
import { Badge, type Tone } from "@/components/ui/badge";
import { DataTable, Tabs } from "@/components/ui/data-table";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";

const FILTERS = [
  ["all", "All"],
  ["due", "Due"],
  ["late", "Late"],
  ["processing", "Processing"],
  ["paid", "Paid"],
  ["failed", "Failed"],
] as const;
type Filter = (typeof FILTERS)[number][0];

function paymentTone(status: string): Tone {
  switch (status) {
    case "paid":
      return "good";
    case "failed":
    case "late":
      return "bad";
    case "processing":
      return "info";
    default:
      return "warn";
  }
}

export default function PaymentsPage() {
  const ready = useReady("payment:read");
  const allowed = useReady();
  const [filter, setFilter] = useState<Filter>("all");
  const params = useMemo(
    () => (filter === "all" ? {} : { status: filter }),
    [filter]
  );
  const q = useQuery({
    queryKey: queryKeys.payments(params),
    queryFn: () => api.payments(params),
    enabled: ready,
  });
  const rows = q.data ?? [];
  const total = rows.reduce((s, p) => s + p.amount_cents, 0);

  if (allowed && !ready) return <NoAccess />;

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Money"
        title="Payments"
        description="Rent, deposits and fees across the workspace. Settled payments post to the ledger and send a receipt."
      />

      <div className="flex flex-wrap items-center justify-between gap-3">
        <Tabs tabs={FILTERS} value={filter} onChange={setFilter} />
        {q.data && (
          <span className="text-[13px] text-fg-3">
            {rows.length} {rows.length === 1 ? "payment" : "payments"} ·{" "}
            <span className="figure text-fg-2">{usd(total)}</span>
          </span>
        )}
      </div>

      {q.error && (
        <Panel className="border-bad/30 p-4 text-[13px] text-bad">
          Couldn&apos;t load payments: {q.error.message}
        </Panel>
      )}

      <Panel className="overflow-hidden">
        {q.isLoading ? (
          <div className="space-y-2 p-4">
            {Array.from({ length: 6 }, (_, i) => (
              <Skeleton key={i} className="h-9" />
            ))}
          </div>
        ) : (
          <DataTable
            columns={[
              "Due",
              "Kind",
              { label: "Amount", num: true },
              "Status",
              "Paid",
              "Receipt",
              "Lease",
            ]}
            rows={rows.map((p) => [
              <span key="d" className="font-mono text-fg-2">
                {p.due_date}
              </span>,
              <span key="k" className="capitalize">
                {p.kind}
              </span>,
              p.amount_label,
              <span key="s" className="inline-flex items-center gap-2">
                <Badge tone={paymentTone(p.status)}>{p.status}</Badge>
                {p.failure_reason && (
                  <span className="text-xs text-bad">{p.failure_reason}</span>
                )}
              </span>,
              <span key="p" className="font-mono">
                {p.paid_date ?? "—"}
                {p.method && (
                  <span className="ml-1 font-sans text-xs text-fg-3">
                    {p.method}
                  </span>
                )}
              </span>,
              <span key="r" className="font-mono text-xs">
                {p.receipt_number ?? "—"}
              </span>,
              <Link
                key="l"
                href={`/console/leases/${p.lease_id}`}
                className="font-medium text-accent hover:underline"
              >
                View lease
              </Link>,
            ])}
            empty={
              filter === "all"
                ? "No payments yet."
                : "No payments match this filter."
            }
          />
        )}
      </Panel>
    </div>
  );
}

function NoAccess() {
  return (
    <div className="space-y-6">
      <PageHeader eyebrow="Money" title="Payments" />
      <Panel>
        <EmptyState
          icon={<Lock />}
          title="You don't have access to payments"
          description="Ask an admin for the payment:read permission."
        />
      </Panel>
    </div>
  );
}
