"use client";

// SaaS billing — platform plane (roadmap Phase 8). Acre HQ's billing console:
// MRR + per-workspace usage, plan management, an on-demand billing run, and the
// cross-tenant invoice ledger with settle / void actions. Staff-only.

import { useCallback, useEffect, useState } from "react";
import { api, type BillingOverview, type PlatformInvoice } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { Badge, Button, Card, StatTile } from "@/components/ui";
import { logError } from "@/lib/log";

const PLAN_KEYS = ["starter", "growth", "enterprise"];

function invoiceTone(status: string): "good" | "warn" | "neutral" {
  if (status === "paid") return "good";
  if (status === "open") return "warn";
  return "neutral";
}

export default function PlatformBillingPage() {
  const { user } = useAuth();
  const [overview, setOverview] = useState<BillingOverview | null>(null);
  const [invoices, setInvoices] = useState<PlatformInvoice[]>([]);
  const [statusFilter, setStatusFilter] = useState("");
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadOverview = useCallback(() => {
    api
      .platformBillingOverview()
      .then(setOverview)
      .catch((e) => {
        logError("failed to load billing overview", e);
        setError(e.message);
      });
  }, []);

  const loadInvoices = useCallback(() => {
    api
      .platformBillingInvoices(statusFilter ? { status: statusFilter } : {})
      .then(setInvoices)
      .catch((e) => logError("failed to load invoices", e));
  }, [statusFilter]);

  useEffect(() => {
    if (!user?.is_platform_staff) return;
    loadOverview();
  }, [user, loadOverview]);

  useEffect(() => {
    if (!user?.is_platform_staff) return;
    loadInvoices();
  }, [user, loadInvoices]);

  if (!user?.is_platform_staff) {
    return (
      <Card className="p-6">
        <p className="text-ink-2">Platform billing is staff-only.</p>
      </Card>
    );
  }

  async function runBilling() {
    setBusy(true);
    setNote(null);
    try {
      const r = await api.platformBillingRun();
      setNote(`Billed ${r.period}: ${r.generated} workspace(s).`);
      loadOverview();
      loadInvoices();
    } catch (e) {
      logError("billing run failed", e);
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }

  async function setPlan(tenantId: string, plan: string) {
    try {
      await api.platformSetPlan(tenantId, plan);
      loadOverview();
    } catch (e) {
      logError("plan change failed", e);
      setError((e as Error).message);
    }
  }

  async function settle(id: string, action: "pay" | "void") {
    try {
      if (action === "pay") await api.platformInvoicePay(id);
      else await api.platformInvoiceVoid(id);
      loadInvoices();
      loadOverview();
    } catch (e) {
      logError("settle failed", e);
      setError((e as Error).message);
    }
  }

  return (
    <div className="space-y-6">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            SaaS billing
          </h1>
          <p className="text-ink-3">
            Metered subscriptions across every client workspace.
          </p>
        </div>
        <Button onClick={runBilling} disabled={busy}>
          {busy ? "Running…" : "Run billing"}
        </Button>
      </div>

      {note && <p className="text-good">{note}</p>}
      {error && <p className="text-bad">{error}</p>}

      {overview && (
        <div className="grid grid-cols-2 gap-3 lg:grid-cols-3">
          <StatTile label="MRR" value={overview.mrr_label} icon="chart" />
          <StatTile
            label="Outstanding"
            value={overview.outstanding_label}
            icon="card"
          />
          <StatTile
            label="Workspaces"
            value={`${overview.tenant_count}`}
            icon="globe"
          />
        </div>
      )}

      {/* Per-workspace subscriptions */}
      <Card className="overflow-hidden">
        <div className="border-b border-line px-5 py-4 font-display text-lg font-bold">
          Workspaces
        </div>
        <div className="divide-y divide-line">
          {overview?.tenants.map((t) => (
            <div
              key={t.tenant_id}
              className="flex items-center gap-4 px-5 py-3.5"
            >
              <div className="min-w-0 flex-1">
                <div className="font-semibold">{t.name}</div>
                <div className="text-sm text-ink-3">
                  {t.slug} · {t.units} units
                </div>
              </div>
              <select
                value={t.plan}
                onChange={(e) => setPlan(t.tenant_id, e.target.value)}
                className="rounded-xl border border-line bg-surface-2 px-3 py-2 text-sm font-semibold capitalize outline-none"
              >
                {PLAN_KEYS.map((k) => (
                  <option key={k} value={k} className="capitalize">
                    {k}
                  </option>
                ))}
              </select>
              <div className="hidden w-24 text-right font-mono text-sm sm:block">
                {t.mrr_label}/mo
              </div>
              <div className="w-24 text-right font-mono text-sm">
                {t.outstanding_cents > 0 ? (
                  <span className="text-warn">{t.outstanding_label} due</span>
                ) : (
                  <span className="text-ink-3">—</span>
                )}
              </div>
            </div>
          ))}
        </div>
      </Card>

      {/* Invoice ledger */}
      <Card className="overflow-hidden">
        <div className="flex items-center justify-between border-b border-line px-5 py-4">
          <div className="font-display text-lg font-bold">Invoices</div>
          <select
            value={statusFilter}
            onChange={(e) => setStatusFilter(e.target.value)}
            className="rounded-xl border border-line bg-surface-2 px-3 py-2 text-sm font-semibold outline-none"
          >
            <option value="">All</option>
            <option value="open">Open</option>
            <option value="paid">Paid</option>
            <option value="void">Void</option>
          </select>
        </div>
        {invoices.length === 0 ? (
          <div className="px-5 py-6 text-sm text-ink-3">No invoices.</div>
        ) : (
          <div className="divide-y divide-line">
            {invoices.map((inv) => (
              <div key={inv.id} className="flex items-center gap-4 px-5 py-3.5">
                <div className="min-w-0 flex-1">
                  <div className="font-semibold">{inv.period}</div>
                  <div className="text-sm capitalize text-ink-3">
                    {inv.plan} · {inv.unit_count} units
                  </div>
                </div>
                <Badge tone={invoiceTone(inv.status)}>{inv.status}</Badge>
                <div className="w-20 text-right font-mono text-sm">
                  {inv.total_label}
                </div>
                {inv.status === "open" ? (
                  <div className="flex gap-2">
                    <Button
                      variant="outline"
                      onClick={() => settle(inv.id, "pay")}
                    >
                      Mark paid
                    </Button>
                    <Button
                      variant="ghost"
                      onClick={() => settle(inv.id, "void")}
                    >
                      Void
                    </Button>
                  </div>
                ) : (
                  <div className="w-[152px]" />
                )}
              </div>
            ))}
          </div>
        )}
      </Card>
    </div>
  );
}
