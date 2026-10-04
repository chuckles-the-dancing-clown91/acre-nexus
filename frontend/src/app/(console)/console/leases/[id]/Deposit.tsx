"use client";

// The security deposit: whether it's held in trust, a draft of move-out
// deductions (lease:manage), and finalize, which posts the ledger entries and
// sends the refund (payout:manage). The statement PDF files on the lease.

import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import { Pencil, Plus, Send, ShieldCheck, X } from "lucide-react";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { Badge, statusTone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { inputClass, useRun } from "../_ui/shared";

interface DraftLine {
  description: string;
  amount: string;
}

export function Deposit({
  leaseId,
  manage,
}: {
  leaseId: string;
  manage: boolean;
}) {
  const { can } = useAuth();
  const canFinalize = can("payout:manage");
  const deposit = useQuery({
    queryKey: ["leases", leaseId, "deposit"],
    queryFn: () => api.leaseDeposit(leaseId),
    // Follow an in-flight refund until it settles.
    refetchInterval: (q) =>
      q.state.data?.disposition?.status === "processing" ? 5000 : false,
  });
  const { busy, run } = useRun([["leases", leaseId, "deposit"]]);
  const [editing, setEditing] = useState(false);
  const [lines, setLines] = useState<DraftLine[]>([]);
  const [notes, setNotes] = useState("");

  if (deposit.isLoading) return <Skeleton className="h-32 rounded-2xl" />;
  if (deposit.error)
    return (
      <Panel className="border-bad/30 p-4 text-[13px] text-bad">
        Couldn&apos;t load the deposit: {deposit.error.message}
      </Panel>
    );
  const dep = deposit.data;
  if (!dep || !dep.deposit_label)
    return (
      <Panel>
        <EmptyState
          icon={<ShieldCheck />}
          title="No deposit on this lease"
          description="Set a deposit when converting the application to a lease."
        />
      </Panel>
    );
  const d = dep.disposition;

  function startEditing() {
    setLines(
      d && d.deductions.length > 0
        ? d.deductions.map((x) => ({
            description: x.description,
            amount: (x.amount_cents / 100).toFixed(2),
          }))
        : [{ description: "", amount: "" }]
    );
    setNotes(d?.notes ?? "");
    setEditing(true);
  }

  async function saveDraft() {
    const deductions: { description: string; amount_cents: number }[] = [];
    for (const l of lines) {
      if (!l.description.trim() && !l.amount.trim()) continue;
      const cents = Math.round(parseFloat(l.amount) * 100);
      if (!l.description.trim() || !Number.isFinite(cents) || cents <= 0) {
        toast.error(
          "Each deduction needs a description and an amount above zero."
        );
        return;
      }
      deductions.push({
        description: l.description.trim(),
        amount_cents: cents,
      });
    }
    const ok = await run(
      "save",
      () =>
        api.saveDepositDisposition(leaseId, {
          deductions,
          notes: notes.trim() || undefined,
        }),
      "Draft saved"
    );
    if (ok) setEditing(false);
  }

  function setLine(idx: number, patch: Partial<DraftLine>) {
    setLines((all) => all.map((x, i) => (i === idx ? { ...x, ...patch } : x)));
  }

  return (
    <Panel>
      <PanelHeader
        title="Security deposit"
        description={
          dep.deposit_paid
            ? "Held in the trust account."
            : "Not settled into trust yet. The resident pays it from their portal, and the refund can be worked out once it's held."
        }
        action={
          <div className="flex items-center gap-2">
            <span className="figure text-[15px] font-semibold text-fg">
              {dep.deposit_label}
            </span>
            <Badge tone={dep.deposit_paid ? "good" : "warn"}>
              {dep.deposit_paid ? "held in trust" : "not collected"}
            </Badge>
          </div>
        }
      />
      <div className="space-y-3 p-5 pt-4 text-[13px]">
        {d && !editing && (
          <div className="rounded-xl border border-line p-3">
            <div className="mb-2 flex items-center justify-between">
              <span className="font-medium text-fg">Move-out refund</span>
              <Badge tone={statusTone(d.status)}>{d.status}</Badge>
            </div>
            {d.deductions.length > 0 ? (
              <ul className="mb-2 space-y-1 text-fg-2">
                {d.deductions.map((x) => (
                  <li key={x.id} className="flex justify-between gap-3">
                    <span>{x.description}</span>
                    <span className="figure">−{x.amount_label}</span>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="mb-2 text-fg-3">No deductions. Full refund.</p>
            )}
            <div className="flex justify-between border-t border-line pt-2 font-semibold text-fg">
              <span>Refund to resident</span>
              <span className="figure">
                {d.refund_label ?? dep.deposit_label}
              </span>
            </div>
            {d.notes && <p className="mt-2 text-fg-3">{d.notes}</p>}
            {d.failure_reason && (
              <p className="mt-2 text-bad" role="alert">
                Refund failed: {d.failure_reason}
              </p>
            )}
          </div>
        )}

        {editing && (
          <form
            className="space-y-2 rounded-xl border border-line bg-fill/40 p-3"
            onSubmit={(e) => {
              e.preventDefault();
              void saveDraft();
            }}
          >
            <div className="text-xs font-medium text-fg-3">Deductions</div>
            {lines.map((l, idx) => (
              <div key={idx} className="flex gap-2">
                <input
                  aria-label="Description"
                  className={inputClass}
                  placeholder="Carpet cleaning"
                  value={l.description}
                  onChange={(e) =>
                    setLine(idx, { description: e.target.value })
                  }
                />
                <input
                  aria-label="Amount"
                  className={`${inputClass} max-w-[120px]`}
                  placeholder="0.00"
                  inputMode="decimal"
                  value={l.amount}
                  onChange={(e) => setLine(idx, { amount: e.target.value })}
                />
                <button
                  type="button"
                  aria-label="Remove deduction"
                  onClick={() => setLines(lines.filter((_, i) => i !== idx))}
                  className="rounded-lg p-2 text-fg-3 transition hover:bg-fill-2 hover:text-bad"
                >
                  <X className="size-4" />
                </button>
              </div>
            ))}
            <Button
              type="button"
              size="sm"
              variant="ghost"
              onClick={() =>
                setLines([...lines, { description: "", amount: "" }])
              }
            >
              <Plus />
              Add deduction
            </Button>
            <textarea
              className={inputClass}
              rows={2}
              placeholder="Notes for the statement (optional)"
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
            />
            <div className="flex justify-end gap-2">
              <Button
                type="button"
                size="sm"
                variant="ghost"
                onClick={() => setEditing(false)}
              >
                Cancel
              </Button>
              <Button type="submit" size="sm" loading={busy === "save"}>
                Save draft
              </Button>
            </div>
          </form>
        )}

        {dep.deposit_paid && manage && !editing && (
          <div className="flex flex-wrap gap-2">
            {(!d || d.status === "draft") && (
              <Button
                size="sm"
                variant="secondary"
                disabled={!!busy}
                onClick={startEditing}
              >
                <Pencil />
                {d ? "Edit deductions" : "Start move-out refund"}
              </Button>
            )}
            {d &&
              (d.status === "draft" || d.status === "failed") &&
              canFinalize && (
                <Button
                  size="sm"
                  loading={busy === "finalize"}
                  onClick={() => {
                    if (
                      !confirm(
                        "Finalize and send the refund? This can't be undone."
                      )
                    )
                      return;
                    void run(
                      "finalize",
                      () => api.finalizeDepositDisposition(d.id),
                      "Finalized. Refund on its way."
                    );
                  }}
                >
                  <Send />
                  {d.status === "failed"
                    ? "Retry refund"
                    : "Finalize and refund"}
                </Button>
              )}
          </div>
        )}
      </div>
    </Panel>
  );
}
