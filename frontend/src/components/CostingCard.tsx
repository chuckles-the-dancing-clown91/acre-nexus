"use client";

// Job costing for one work order (or rehab project): what's ready to bill the
// owner, previous bills, and — with payroll:read — the full cost and profit
// picture (labor, parts, mileage, expenses, margin vs. target).

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { toast } from "sonner";
import {
  costing,
  hm,
  money,
  openPdf,
  pct,
  type BillPreview,
  type Costs,
  type WorkKind,
} from "@/lib/backoffice";
import { useAuth } from "@/lib/auth";
import { Badge, Button, Card, statusTone } from "@/components/ui";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

const field =
  "w-full rounded-xl border border-line bg-surface px-3 py-2 text-sm outline-none focus:border-accent";

export function CostingCard({ kind, id }: { kind: WorkKind; id: string }) {
  const { can } = useAuth();
  const teamRead = can("team:read");
  const payroll = can("payroll:read");
  const canBill = can("team:manage") && can("payable:manage");

  const [preview, setPreview] = useState<BillPreview | null>(null);
  const [costs, setCosts] = useState<Costs | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [memo, setMemo] = useState("");
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    if (!teamRead) return;
    costing
      .preview(kind, id)
      .then(setPreview)
      .catch((e) =>
        setError(e instanceof Error ? e.message : "Couldn't load costs")
      );
    if (payroll)
      costing
        .costs(kind, id)
        .then(setCosts)
        .catch((e) =>
          setError(e instanceof Error ? e.message : "Couldn't load costs")
        );
  }, [kind, id, teamRead, payroll]);

  useEffect(() => {
    load();
  }, [load]);

  if (!teamRead) return null;

  async function billOwner() {
    setBusy(true);
    try {
      const res = await costing.billOwner(kind, id, {
        memo: memo.trim() || undefined,
      });
      toast.success(
        `Draft bill ${res.bill_number} created — submit it in Payables`
      );
      setConfirming(false);
      setMemo("");
      load();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't bill the owner");
    } finally {
      setBusy(false);
    }
  }

  async function printSheet() {
    try {
      await openPdf(costing.sheetPath(kind, id));
    } catch (e) {
      toast.error(
        e instanceof Error ? e.message : "Couldn't open the cost sheet"
      );
    }
  }

  const hasLines = !!preview && preview.lines.length > 0;

  return (
    <Card>
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-line px-5 py-4">
        <h2 className="font-display text-lg font-bold">Job costing</h2>
        <div className="flex flex-wrap gap-2">
          <Button variant="outline" onClick={() => void printSheet()}>
            Print cost sheet
          </Button>
          {canBill && (
            <Button
              disabled={!hasLines || busy}
              onClick={() => setConfirming(true)}
              title={hasLines ? undefined : "Nothing new to bill yet"}
            >
              Bill the owner
            </Button>
          )}
        </div>
      </div>

      <div className="space-y-5 p-5 text-sm">
        {error && <p className="text-bad">{error}</p>}
        {!preview && !error && <p className="text-ink-3">Loading…</p>}

        {preview && (
          <div className="space-y-2">
            <div className="text-xs font-semibold uppercase tracking-wide text-ink-3">
              To bill the owner
              {preview.markup_bps > 0 && (
                <span className="ml-2 normal-case">
                  (includes {pct(preview.markup_bps)} markup)
                </span>
              )}
            </div>
            {preview.lines.length === 0 ? (
              <p className="text-ink-3">
                Nothing new to bill — approved time, parts and billable expenses
                show up here.
              </p>
            ) : (
              <BillLines lines={preview.lines} total={preview.total_cents} />
            )}
            {preview.held_back.length > 0 && (
              <div className="rounded-xl border border-line-2 bg-warn-soft px-3 py-2">
                <div className="font-semibold text-warn">Held back</div>
                <ul className="mt-1 list-disc pl-5 text-ink-2">
                  {preview.held_back.map((h, i) => (
                    <li key={i}>{h}</li>
                  ))}
                </ul>
              </div>
            )}
            {preview.previous_bills.length > 0 && (
              <div className="space-y-1 pt-1">
                <div className="text-xs font-semibold uppercase tracking-wide text-ink-3">
                  Already billed
                </div>
                {preview.previous_bills.map((b) => (
                  <div
                    key={b.id}
                    className="flex items-center justify-between gap-3 rounded-xl border border-line px-3 py-2"
                  >
                    <Link href="/console/payables" className="font-semibold">
                      Bill {b.bill_number}
                    </Link>
                    <span className="flex items-center gap-2">
                      <Badge tone={statusTone(b.status)}>{b.status}</Badge>
                      <span className="font-mono">{money(b.amount_cents)}</span>
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {costs && <ProfitPanel c={costs} />}
      </div>

      <Dialog open={confirming} onOpenChange={setConfirming}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Bill the owner</DialogTitle>
            <DialogDescription>
              This creates a draft bill in Payables. Nothing is charged until
              you submit it there.
            </DialogDescription>
          </DialogHeader>
          {preview && (
            <BillLines lines={preview.lines} total={preview.total_cents} />
          )}
          <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
            Memo (optional)
            <input
              className={field}
              placeholder="e.g. Water heater replacement, unit 2"
              value={memo}
              onChange={(e) => setMemo(e.target.value)}
            />
          </label>
          <DialogFooter>
            <Button variant="outline" onClick={() => setConfirming(false)}>
              Cancel
            </Button>
            <Button disabled={busy} onClick={() => void billOwner()}>
              {busy ? "Creating…" : "Create draft bill"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </Card>
  );
}

function BillLines({
  lines,
  total,
}: {
  lines: { description: string; amount_cents: number }[];
  total: number;
}) {
  return (
    <div className="divide-y divide-line rounded-xl border border-line text-sm">
      {lines.map((l, i) => (
        <div key={i} className="flex justify-between gap-3 px-3 py-2">
          <span>{l.description}</span>
          <span className="font-mono">{money(l.amount_cents)}</span>
        </div>
      ))}
      <div className="flex justify-between gap-3 px-3 py-2 font-bold">
        <span>Total</span>
        <span className="font-mono">{money(total)}</span>
      </div>
    </div>
  );
}

function Row({
  label,
  value,
  sub,
  strong,
}: {
  label: string;
  value: string;
  sub?: boolean;
  strong?: boolean;
}) {
  return (
    <div
      className={`flex justify-between gap-3 ${sub ? "pl-4 text-xs text-ink-3" : ""} ${strong ? "font-bold" : ""}`}
    >
      <span>{label}</span>
      <span className="font-mono">{value}</span>
    </div>
  );
}

function ProfitPanel({ c }: { c: Costs }) {
  return (
    <div className="space-y-3 border-t border-line pt-4">
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-xs font-semibold uppercase tracking-wide text-ink-3">
          Cost & profit
        </span>
        {c.under_target && c.revenue_cents > 0 && (
          <Badge tone="warn">under {pct(c.target_margin_bps)} target</Badge>
        )}
      </div>
      <div className="grid gap-5 md:grid-cols-2">
        <div className="space-y-1">
          <Row label="Hours" value={hm(c.minutes)} />
          <Row label="Labor" value={money(c.labor_cents)} />
          <Row label="Pay" value={money(c.labor_pay_cents)} sub />
          <Row
            label="Overtime premium"
            value={money(c.overtime_premium_cents)}
            sub
          />
          <Row label="Burden (taxes, comp)" value={money(c.burden_cents)} sub />
          <Row label="Parts" value={money(c.parts_cents)} />
          {c.other_lines_cents !== 0 && (
            <Row label="Other lines" value={money(c.other_lines_cents)} />
          )}
          <Row label="Mileage" value={money(c.mileage_cents)} />
          <Row label="Expenses" value={money(c.expenses_cents)} />
          <Row label="Total cost" value={money(c.costs_cents)} strong />
        </div>
        <div className="space-y-1">
          <Row label="Billed" value={money(c.billed_cents)} />
          <Row label="Not billed yet" value={money(c.unbilled_cents)} />
          <Row
            label={`Gross profit (${pct(c.gross_bps)})`}
            value={money(c.gross_cents)}
            strong
          />
          {c.overhead_cents !== 0 && (
            <Row label="Overhead" value={money(c.overhead_cents)} sub />
          )}
          <Row label="Net" value={money(c.net_cents)} />
          <Row
            label="Outside vendor bills"
            value={money(c.vendor_bills_cents)}
          />
          <Row
            label="Total to owner"
            value={money(c.owner_total_cents)}
            strong
          />
          <Row
            label={`Bill rate to hit ${pct(c.target_margin_bps)}`}
            value={`${money(c.bill_rate_for_target_cents)}/h`}
          />
        </div>
      </div>
      {c.by_person.length > 0 && (
        <div className="space-y-1">
          <div className="text-xs font-semibold uppercase tracking-wide text-ink-3">
            Time by person
          </div>
          <div className="flex flex-wrap gap-2">
            {c.by_person.map((p) => (
              <span
                key={p.user_id}
                className="rounded-lg bg-surface-2 px-2.5 py-1 text-xs"
              >
                <span className="font-semibold">{p.name}</span>{" "}
                <span className="font-mono">{hm(p.minutes)}</span>
                {p.billable_cents > 0 && (
                  <span className="text-ink-3">
                    {" "}
                    · {money(p.billable_cents)}
                  </span>
                )}
              </span>
            ))}
          </div>
        </div>
      )}
      {c.overtime_rule && (
        <p className="text-xs text-ink-3">{c.overtime_rule}</p>
      )}
    </div>
  );
}
