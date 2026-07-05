"use client";

// Owner payouts: compute a draft from the entity's actual books for a period,
// review the math (rent collected − expenses − management fee), execute it as
// an ACH transfer, and follow settlement (ledger entry + statement).

import { useMemo, useState } from "react";
import { useAuth } from "@/lib/auth";
import {
  useComputePayout,
  useEntityPicker,
  useExecutePayout,
  usePayouts,
} from "@/lib/queries";
import { Badge, Button, Card } from "@/components/ui";

function payoutTone(status: string) {
  switch (status) {
    case "paid":
      return "good" as const;
    case "failed":
      return "bad" as const;
    case "processing":
      return "info" as const;
    default:
      return "warn" as const;
  }
}

/** First + last day of the previous month, `YYYY-MM-DD`. */
function lastMonthBounds(): { start: string; end: string } {
  const now = new Date();
  const first = new Date(
    Date.UTC(now.getUTCFullYear(), now.getUTCMonth() - 1, 1)
  );
  const last = new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), 0));
  const iso = (d: Date) => d.toISOString().slice(0, 10);
  return { start: iso(first), end: iso(last) };
}

export default function PayoutsPage() {
  const { can } = useAuth();
  const { entities, defaultId } = useEntityPicker();
  const { data: payouts } = usePayouts({
    // Follow in-flight payouts to settlement.
    refetchInterval: (q) =>
      q.state.data?.some((p) => p.status === "processing") ? 4000 : false,
  });
  const compute = useComputePayout();
  const execute = useExecutePayout();

  const defaults = useMemo(lastMonthBounds, []);
  const [entityId, setEntityId] = useState<string | undefined>(undefined);
  const [start, setStart] = useState(defaults.start);
  const [end, setEnd] = useState(defaults.end);
  const activeEntity = entityId ?? defaultId;
  const manage = can("payout:manage");

  if (!can("ledger:read")) {
    return (
      <p className="text-ink-3">
        You don&apos;t have permission to view payouts.
      </p>
    );
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="font-display text-3xl font-extrabold tracking-tight">
          Owner payouts
        </h1>
        <p className="text-ink-3">
          From rent collected to owner paid: computed from the ledger, executed
          by ACH, documented with a statement.
        </p>
      </div>

      {manage && (
        <Card className="flex flex-wrap items-end gap-3 p-5">
          <label className="text-sm font-semibold text-ink-2">
            Entity
            <select
              value={activeEntity ?? ""}
              onChange={(e) => setEntityId(e.target.value)}
              className="mt-1 block rounded-xl border border-line bg-surface px-3 py-2 font-semibold"
            >
              {entities?.map((e) => (
                <option key={e.id} value={e.id}>
                  {e.name}
                </option>
              ))}
            </select>
          </label>
          <label className="text-sm font-semibold text-ink-2">
            From
            <input
              type="date"
              value={start}
              onChange={(e) => setStart(e.target.value)}
              className="mt-1 block rounded-xl border border-line bg-surface px-3 py-2"
            />
          </label>
          <label className="text-sm font-semibold text-ink-2">
            To
            <input
              type="date"
              value={end}
              onChange={(e) => setEnd(e.target.value)}
              className="mt-1 block rounded-xl border border-line bg-surface px-3 py-2"
            />
          </label>
          <Button
            disabled={!activeEntity || compute.isPending}
            onClick={() =>
              activeEntity &&
              compute.mutate({
                entity_id: activeEntity,
                period_start: start,
                period_end: end,
              })
            }
          >
            Compute payout
          </Button>
        </Card>
      )}

      <div className="space-y-3">
        {payouts?.map((p) => (
          <Card key={p.id} className="px-5 py-4">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div>
                <div className="font-semibold">
                  {p.entity_name ?? "Entity"}{" "}
                  <span className="font-mono text-sm text-ink-3">
                    {p.period_start} → {p.period_end}
                  </span>
                </div>
                <div className="mt-1 text-sm text-ink-2">
                  Collected {p.rent_collected_label} · expenses{" "}
                  {p.expenses_label} · mgmt fee {p.mgmt_fee_label}
                </div>
                {p.failure_reason && (
                  <div className="mt-1 text-sm text-bad">
                    {p.failure_reason}
                  </div>
                )}
              </div>
              <div className="flex items-center gap-3">
                <div className="text-right">
                  <div className="text-xs uppercase tracking-wide text-ink-3">
                    Net draw
                  </div>
                  <div className="font-display text-xl font-extrabold">
                    {p.net_label}
                  </div>
                </div>
                <Badge tone={payoutTone(p.status)}>{p.status}</Badge>
                {manage && (p.status === "draft" || p.status === "failed") && (
                  <Button
                    disabled={execute.isPending || p.net_cents <= 0}
                    onClick={() => execute.mutate(p.id)}
                  >
                    Execute
                  </Button>
                )}
              </div>
            </div>
            {p.status === "paid" && (
              <div className="mt-2 text-xs text-ink-3">
                Settled — ledger entry {p.ledger_txn_id ? "posted" : "pending"}
                {p.statement_document_id
                  ? " · statement filed on the entity"
                  : ""}
              </div>
            )}
          </Card>
        ))}
        {payouts && payouts.length === 0 && (
          <Card className="px-5 py-10 text-center text-ink-3">
            No payouts yet — compute one from a period above.
          </Card>
        )}
      </div>
    </div>
  );
}
