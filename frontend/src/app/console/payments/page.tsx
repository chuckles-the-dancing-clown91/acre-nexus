"use client";

// Back-office payments view: tenant-wide rent collection status. Rows are the
// lease_payment lifecycle — due → processing → paid/failed (+ late) — with
// receipts and failure reasons surfaced inline.

import { useState } from "react";
import Link from "next/link";
import { useAuth } from "@/lib/auth";
import { usePayments } from "@/lib/queries";
import { Badge, Card } from "@/components/ui";

const FILTERS = ["all", "due", "late", "processing", "paid", "failed"] as const;

function paymentTone(status: string) {
  switch (status) {
    case "paid":
      return "good" as const;
    case "failed":
    case "late":
      return "bad" as const;
    case "processing":
      return "info" as const;
    default:
      return "warn" as const;
  }
}

export default function PaymentsPage() {
  const { can } = useAuth();
  const [filter, setFilter] = useState<(typeof FILTERS)[number]>("all");
  const { data: payments } = usePayments(
    filter === "all" ? {} : { status: filter },
    { enabled: can("payment:read") }
  );

  if (!can("payment:read")) {
    return (
      <p className="text-ink-3">
        You don&apos;t have permission to view payments.
      </p>
    );
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="font-display text-3xl font-extrabold tracking-tight">
          Payments
        </h1>
        <p className="text-ink-3">
          Rent, deposits, and fees collected across the workspace — settled
          payments post to the ledger and issue receipts automatically.
        </p>
      </div>

      <div className="flex flex-wrap gap-2">
        {FILTERS.map((f) => (
          <button
            key={f}
            onClick={() => setFilter(f)}
            className={
              f === filter
                ? "rounded-xl bg-accent px-4 py-2 text-sm font-bold capitalize text-on-accent"
                : "rounded-xl border border-line px-4 py-2 text-sm font-bold capitalize text-ink-2 hover:bg-surface-2"
            }
          >
            {f}
          </button>
        ))}
      </div>

      <Card className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-line text-left text-xs uppercase tracking-wide text-ink-3">
              <th className="px-5 py-3">Due</th>
              <th className="px-5 py-3">Kind</th>
              <th className="px-5 py-3 text-right">Amount</th>
              <th className="px-5 py-3">Status</th>
              <th className="px-5 py-3">Paid</th>
              <th className="px-5 py-3">Receipt</th>
              <th className="px-5 py-3">Lease</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-line">
            {payments?.map((p) => (
              <tr key={p.id}>
                <td className="px-5 py-3 font-mono text-ink-2">{p.due_date}</td>
                <td className="px-5 py-3 capitalize">{p.kind}</td>
                <td className="px-5 py-3 text-right font-mono">
                  {p.amount_label}
                </td>
                <td className="px-5 py-3">
                  <Badge tone={paymentTone(p.status)}>{p.status}</Badge>
                  {p.failure_reason && (
                    <span className="ml-2 text-xs text-bad">
                      {p.failure_reason}
                    </span>
                  )}
                </td>
                <td className="px-5 py-3 font-mono text-ink-2">
                  {p.paid_date ?? "—"}
                  {p.method && (
                    <span className="ml-1 text-xs text-ink-3">
                      ({p.method})
                    </span>
                  )}
                </td>
                <td className="px-5 py-3 font-mono text-xs text-ink-2">
                  {p.receipt_number ?? "—"}
                </td>
                <td className="px-5 py-3">
                  <Link
                    href={`/console/leases/${p.lease_id}`}
                    className="font-semibold text-accent hover:underline"
                  >
                    View lease
                  </Link>
                </td>
              </tr>
            ))}
            {payments && payments.length === 0 && (
              <tr>
                <td colSpan={7} className="px-5 py-10 text-center text-ink-3">
                  No payments match this filter.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </Card>
    </div>
  );
}
