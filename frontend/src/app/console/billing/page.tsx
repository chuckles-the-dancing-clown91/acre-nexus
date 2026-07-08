"use client";

// SaaS billing — workspace self-serve (roadmap Phase 8). A workspace views its
// subscription (plan, live unit meter, and the charge estimated for the period
// in progress), compares plans, and downloads its platform invoices. Read-only:
// plan changes are handled by Acre HQ. Gated by `billing:read`.

import { useEffect, useState } from "react";
import { api, type BillingSubscription, type PlatformInvoice } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { Badge, Button, Card, StatTile } from "@/components/ui";
import { Icon } from "@/components/Icon";
import { logError } from "@/lib/log";

function invoiceTone(status: string): "good" | "warn" | "neutral" {
  if (status === "paid") return "good";
  if (status === "open") return "warn";
  return "neutral";
}

function periodLabel(period: string): string {
  const [y, m] = period.split("-").map(Number);
  if (!y || !m) return period;
  return new Date(y, m - 1, 1).toLocaleString("en-US", {
    month: "long",
    year: "numeric",
  });
}

async function downloadInvoice(id: string, period: string) {
  try {
    const blob = await api.downloadReport(
      `/billing/invoices/${id}/export?format=pdf`
    );
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `invoice-${period}.pdf`;
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
  } catch (e) {
    logError("failed to download invoice", e);
  }
}

export default function BillingPage() {
  const { can } = useAuth();
  const [sub, setSub] = useState<BillingSubscription | null>(null);
  const [invoices, setInvoices] = useState<PlatformInvoice[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!can("billing:read")) return;
    Promise.all([api.billingSubscription(), api.billingInvoices()])
      .then(([s, inv]) => {
        setSub(s);
        setInvoices(inv);
      })
      .catch((e) => {
        logError("failed to load billing", e);
        setError(e.message);
      });
  }, [can]);

  if (!can("billing:read")) {
    return (
      <Card className="p-6">
        <p className="text-ink-2">
          You don&apos;t have access to billing for this workspace.
        </p>
      </Card>
    );
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="font-display text-3xl font-extrabold tracking-tight">
          Billing &amp; subscription
        </h1>
        <p className="text-ink-3">
          Your Acre Nexus plan, usage, and platform invoices.
        </p>
      </div>

      {error && <p className="text-bad">{error}</p>}

      {sub && (
        <>
          <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
            <StatTile label="Plan" value={sub.plan_name} icon="shield" />
            <StatTile
              label="Units under mgmt"
              value={`${sub.units}`}
              icon="building"
            />
            <StatTile
              label="This period"
              value={sub.estimate.total_label}
              icon="dollar"
            />
            <StatTile
              label="Outstanding"
              value={sub.outstanding_label}
              icon="card"
            />
          </div>

          {/* Current-period estimate breakdown */}
          <Card className="overflow-hidden">
            <div className="flex items-center justify-between border-b border-line px-5 py-4">
              <div className="font-display text-lg font-bold">
                Estimated charge this period
              </div>
              <Badge tone="info">
                {sub.estimate.unit_count} units · {sub.estimate.included_units}{" "}
                included
              </Badge>
            </div>
            <div className="divide-y divide-line">
              {sub.estimate.lines.map((l, i) => (
                <div
                  key={i}
                  className="flex items-center justify-between px-5 py-3"
                >
                  <span className="text-sm">{l.description}</span>
                  <span className="font-mono text-sm">{l.amount_label}</span>
                </div>
              ))}
              <div className="flex items-center justify-between bg-surface-2 px-5 py-3 font-bold">
                <span>Total</span>
                <span className="font-mono">{sub.estimate.total_label}</span>
              </div>
            </div>
          </Card>

          {/* Plan comparison */}
          <div>
            <h2 className="mb-3 font-display text-xl font-bold">Plans</h2>
            <div className="grid gap-3 md:grid-cols-3">
              {sub.plans.map((p) => (
                <Card
                  key={p.key}
                  className={
                    p.current ? "border-accent p-5 ring-1 ring-accent" : "p-5"
                  }
                >
                  <div className="mb-1 flex items-center justify-between">
                    <div className="font-display text-lg font-bold">
                      {p.name}
                    </div>
                    {p.current && <Badge tone="accent">Current</Badge>}
                  </div>
                  <div className="font-display text-2xl font-extrabold tracking-tight">
                    {p.base_label}
                  </div>
                  <div className="text-xs text-ink-3">
                    {p.included_units} units included · then {p.overage_label}
                  </div>
                  <p className="mt-2 text-sm text-ink-2">{p.blurb}</p>
                  <ul className="mt-3 space-y-1.5">
                    {p.features.map((f) => (
                      <li
                        key={f}
                        className="flex items-start gap-2 text-sm text-ink-2"
                      >
                        <Icon name="check" size={15} />
                        <span>{f}</span>
                      </li>
                    ))}
                  </ul>
                </Card>
              ))}
            </div>
            <p className="mt-2 text-xs text-ink-3">
              To change plans, contact your Acre account manager.
            </p>
          </div>

          {/* Invoice history */}
          <Card className="overflow-hidden">
            <div className="border-b border-line px-5 py-4 font-display text-lg font-bold">
              Invoices
            </div>
            {invoices.length === 0 ? (
              <div className="px-5 py-6 text-sm text-ink-3">
                No invoices yet — your first bill is generated at the end of the
                billing month.
              </div>
            ) : (
              <div className="divide-y divide-line">
                {invoices.map((inv) => (
                  <div
                    key={inv.id}
                    className="flex items-center gap-4 px-5 py-3.5"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="font-semibold">
                        {periodLabel(inv.period)}
                      </div>
                      <div className="text-sm text-ink-3">
                        {inv.unit_count} units
                        {inv.due_date ? ` · due ${inv.due_date}` : ""}
                      </div>
                    </div>
                    <Badge tone={invoiceTone(inv.status)}>{inv.status}</Badge>
                    <div className="w-20 text-right font-mono text-sm">
                      {inv.total_label}
                    </div>
                    <Button
                      variant="outline"
                      onClick={() => downloadInvoice(inv.id, inv.period)}
                    >
                      PDF
                    </Button>
                  </div>
                ))}
              </div>
            )}
          </Card>
        </>
      )}
    </div>
  );
}
