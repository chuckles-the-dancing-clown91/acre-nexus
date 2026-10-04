"use client";

// Owner payouts: compute a draft from an entity's books for a period (rent
// collected, less expenses and the management fee), send it by ACH, and
// follow it to settlement, when the ledger entry and statement are filed.

import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Calculator, Coins, Lock, Send } from "lucide-react";
import { toast } from "sonner";
import { api, type Payout } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { queryKeys } from "@/lib/queries";
import { isoDate } from "@/lib/backoffice";
import { errMsg, useEntityChoice, useReady } from "@/lib/money-extra";
import { Badge, type Tone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { fieldClass, Label } from "@/components/ui/input";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";

function payoutTone(status: string): Tone {
  switch (status) {
    case "paid":
      return "good";
    case "failed":
      return "bad";
    case "processing":
      return "info";
    default:
      return "warn";
  }
}

/** First and last day of last month. */
function lastMonth(): { start: string; end: string } {
  const now = new Date();
  return {
    start: isoDate(new Date(now.getFullYear(), now.getMonth() - 1, 1)),
    end: isoDate(new Date(now.getFullYear(), now.getMonth(), 0)),
  };
}

export default function PayoutsPage() {
  const { can } = useAuth();
  const ready = useReady("ledger:read");
  const allowed = useReady();
  const manage = can("payout:manage");
  const qc = useQueryClient();
  const payouts = useQuery({
    queryKey: queryKeys.payouts,
    queryFn: () => api.payouts(),
    enabled: ready,
    // Follow in-flight payouts to settlement.
    refetchInterval: (q) =>
      q.state.data?.some((p) => p.status === "processing") ? 4000 : false,
  });
  const execute = useMutation({
    mutationFn: (id: string) => api.executePayout(id),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: queryKeys.payouts });
      toast.success("Payout sent. The statement files when it settles.");
    },
    onError: (e) => toast.error(errMsg(e, "Couldn't send the payout")),
  });

  if (allowed && !ready)
    return (
      <div className="space-y-6">
        <PageHeader eyebrow="Money" title="Owner payouts" />
        <Panel>
          <EmptyState
            icon={<Lock />}
            title="You don't have access to payouts"
            description="Ask an admin for the ledger:read permission."
          />
        </Panel>
      </div>
    );

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Money"
        title="Owner payouts"
        description="From rent collected to the owner paid: worked out from the ledger, sent by ACH, filed with a statement."
      />

      {manage && <ComputeForm enabled={ready} />}

      {payouts.error && (
        <Panel className="border-bad/30 p-4 text-[13px] text-bad">
          Couldn&apos;t load payouts: {payouts.error.message}
        </Panel>
      )}

      {payouts.isLoading && (
        <div className="space-y-3">
          {Array.from({ length: 3 }, (_, i) => (
            <Skeleton key={i} className="h-24 rounded-2xl" />
          ))}
        </div>
      )}

      {payouts.data && payouts.data.length === 0 && (
        <Panel>
          <EmptyState
            icon={<Coins />}
            title="No payouts yet"
            description={
              manage
                ? "Compute one for an entity and a period above."
                : "Payouts show here once the office computes them."
            }
          />
        </Panel>
      )}

      <div className="space-y-3">
        {payouts.data?.map((p) => (
          <PayoutRow
            key={p.id}
            p={p}
            manage={manage}
            busy={execute.isPending}
            onExecute={() => execute.mutate(p.id)}
          />
        ))}
      </div>
    </div>
  );
}

function ComputeForm({ enabled }: { enabled: boolean }) {
  const qc = useQueryClient();
  const { entities, defaultId } = useEntityChoice(enabled);
  const [entityId, setEntityId] = useState<string | undefined>(undefined);
  const [{ start: s0, end: e0 }] = useState(lastMonth);
  const [start, setStart] = useState(s0);
  const [end, setEnd] = useState(e0);
  const active = entityId ?? defaultId;
  const compute = useMutation({
    mutationFn: () =>
      api.computePayout({
        entity_id: active!,
        period_start: start,
        period_end: end,
      }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: queryKeys.payouts });
      toast.success("Payout worked out from the ledger");
    },
    onError: (e) => toast.error(errMsg(e, "Couldn't compute the payout")),
  });

  return (
    <Panel>
      <PanelHeader
        title="Compute a payout"
        description="Draft one from the entity's books. Review it, then send it."
      />
      <div className="flex flex-wrap items-end gap-3 p-5">
        <div className="space-y-1.5">
          <Label htmlFor="payout-entity">Entity</Label>
          <select
            id="payout-entity"
            className={`${fieldClass} block min-w-52`}
            value={active ?? ""}
            onChange={(e) => setEntityId(e.target.value)}
          >
            {(entities ?? []).map((e) => (
              <option key={e.id} value={e.id}>
                {e.name}
              </option>
            ))}
          </select>
        </div>
        <div className="space-y-1.5">
          <Label htmlFor="payout-from">From</Label>
          <input
            id="payout-from"
            type="date"
            className={`${fieldClass} block`}
            value={start}
            onChange={(e) => setStart(e.target.value)}
          />
        </div>
        <div className="space-y-1.5">
          <Label htmlFor="payout-to">To</Label>
          <input
            id="payout-to"
            type="date"
            className={`${fieldClass} block`}
            value={end}
            min={start}
            onChange={(e) => setEnd(e.target.value)}
          />
        </div>
        <Button
          disabled={!active || !start || !end}
          loading={compute.isPending}
          onClick={() => compute.mutate()}
        >
          <Calculator />
          Compute payout
        </Button>
      </div>
    </Panel>
  );
}

function PayoutRow({
  p,
  manage,
  busy,
  onExecute,
}: {
  p: Payout;
  manage: boolean;
  busy: boolean;
  onExecute: () => void;
}) {
  return (
    <Panel className="px-5 py-4">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-[14px] font-semibold text-fg">
              {p.entity_name ?? "Entity"}
            </span>
            <span className="figure text-xs text-fg-3">
              {p.period_start} to {p.period_end}
            </span>
          </div>
          <div className="mt-1 text-[13px] text-fg-2">
            Collected {p.rent_collected_label} · expenses {p.expenses_label} ·
            management fee {p.mgmt_fee_label}
          </div>
          {p.failure_reason && (
            <div className="mt-1 text-[13px] text-bad">{p.failure_reason}</div>
          )}
          {p.status === "paid" && (
            <div className="mt-1 text-xs text-fg-3">
              Settled · ledger entry {p.ledger_txn_id ? "posted" : "pending"}
              {p.statement_document_id
                ? " · statement filed on the entity"
                : ""}
            </div>
          )}
        </div>
        <div className="flex items-center gap-3">
          <div className="text-right">
            <div className="eyebrow">Net to owner</div>
            <div className="figure mt-1 text-[20px] leading-none font-semibold text-fg">
              {p.net_label}
            </div>
          </div>
          <Badge tone={payoutTone(p.status)}>{p.status}</Badge>
          {manage && (p.status === "draft" || p.status === "failed") && (
            <Button
              size="sm"
              disabled={busy || p.net_cents <= 0}
              onClick={onExecute}
            >
              <Send />
              {p.status === "failed" ? "Retry" : "Send"}
            </Button>
          )}
        </div>
      </div>
    </Panel>
  );
}
