"use client";

// Security-deposit disposition for one lease (Phase 5): the deposit's trust
// status, an editable deduction draft (`lease:manage`), and finalize —
// which posts the ledger entries and executes the refund transfer
// (`payout:manage`). The statement PDF files on the lease's documents.

import { useCallback, useEffect, useState } from "react";
import { api, type LeaseDeposit } from "@/lib/api";
import { toast } from "sonner";
import { useAuth } from "@/lib/auth";
import { Badge, Button, Card, statusTone } from "@/components/ui";

const field =
  "w-full rounded-xl border border-line bg-surface px-3 py-2 text-sm outline-none focus:border-accent";

interface DraftLine {
  description: string;
  amount: string;
}

export function DepositCard({
  leaseId,
  manage,
}: {
  leaseId: string;
  manage: boolean;
}) {
  const { can } = useAuth();
  const canFinalize = can("payout:manage");
  const [deposit, setDeposit] = useState<LeaseDeposit | null>(null);
  const [editing, setEditing] = useState(false);
  const [lines, setLines] = useState<DraftLine[]>([]);
  const [notes, setNotes] = useState("");
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    api
      .leaseDeposit(leaseId)
      .then((d) => {
        setDeposit(d);
        // Refresh follows an in-flight refund to its settled state.
        if (d.disposition?.status === "processing") {
          setTimeout(
            () =>
              api
                .leaseDeposit(leaseId)
                .then(setDeposit)
                .catch(() => {}),
            5000
          );
        }
      })
      .catch(() => setDeposit(null));
  }, [leaseId]);

  useEffect(() => {
    load();
  }, [load]);

  if (!deposit || !deposit.deposit_label) return null;
  const d = deposit.disposition;

  async function run(fn: () => Promise<unknown>, ok?: string) {
    setBusy(true);
    try {
      await fn();
      if (ok) toast.success(ok);
      setEditing(false);
      load();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Request failed");
    } finally {
      setBusy(false);
    }
  }

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
          "Every deduction needs a description and a positive amount."
        );
        return;
      }
      deductions.push({
        description: l.description.trim(),
        amount_cents: cents,
      });
    }
    await run(
      () =>
        api.saveDepositDisposition(leaseId, {
          deductions,
          notes: notes.trim() || undefined,
        }),
      "Disposition draft saved."
    );
  }

  return (
    <Card>
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-line px-5 py-4">
        <h2 className="font-display text-lg font-bold">Security deposit</h2>
        <div className="flex items-center gap-2">
          <span className="font-mono text-sm">{deposit.deposit_label}</span>
          <Badge tone={deposit.deposit_paid ? "good" : "warn"}>
            {deposit.deposit_paid ? "held in trust" : "not collected"}
          </Badge>
        </div>
      </div>
      <div className="space-y-3 px-5 py-4 text-sm">
        {!deposit.deposit_paid && (
          <p className="text-ink-3">
            The deposit hasn&apos;t settled into trust yet — the resident pays
            it from their portal. Disposition unlocks once it&apos;s held.
          </p>
        )}

        {d && !editing && (
          <div className="rounded-xl border border-line p-4">
            <div className="mb-2 flex items-center justify-between">
              <span className="font-semibold">Move-out disposition</span>
              <Badge tone={statusTone(d.status)}>{d.status}</Badge>
            </div>
            {d.deductions.length > 0 ? (
              <ul className="mb-2 space-y-1 text-ink-2">
                {d.deductions.map((x) => (
                  <li key={x.id} className="flex justify-between">
                    <span>{x.description}</span>
                    <span className="font-mono">−{x.amount_label}</span>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="mb-2 text-ink-3">No deductions — full refund.</p>
            )}
            <div className="flex justify-between font-semibold">
              <span>Refund to resident</span>
              <span className="font-mono">
                {d.refund_label ?? deposit.deposit_label}
              </span>
            </div>
            {d.failure_reason && (
              <p className="mt-2 text-bad" role="alert">
                Refund failed: {d.failure_reason}
              </p>
            )}
          </div>
        )}

        {editing && (
          <div className="space-y-2 rounded-xl border border-line p-4">
            <div className="font-semibold">Deductions</div>
            {lines.map((l, idx) => (
              <div key={idx} className="flex gap-2">
                <input
                  className={field}
                  placeholder="Description, e.g. “Carpet cleaning”"
                  value={l.description}
                  onChange={(e) =>
                    setLines(
                      lines.map((x, i) =>
                        i === idx ? { ...x, description: e.target.value } : x
                      )
                    )
                  }
                />
                <input
                  className={`${field} max-w-[130px]`}
                  placeholder="0.00"
                  inputMode="decimal"
                  value={l.amount}
                  onChange={(e) =>
                    setLines(
                      lines.map((x, i) =>
                        i === idx ? { ...x, amount: e.target.value } : x
                      )
                    )
                  }
                />
                <Button
                  variant="ghost"
                  type="button"
                  onClick={() => setLines(lines.filter((_, i) => i !== idx))}
                >
                  ✕
                </Button>
              </div>
            ))}
            <Button
              variant="outline"
              type="button"
              onClick={() =>
                setLines([...lines, { description: "", amount: "" }])
              }
            >
              Add deduction
            </Button>
            <textarea
              className={field}
              rows={2}
              placeholder="Notes for the statement (optional)"
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
            />
            <div className="flex gap-2">
              <Button disabled={busy} onClick={() => void saveDraft()}>
                Save draft
              </Button>
              <Button
                variant="ghost"
                type="button"
                onClick={() => setEditing(false)}
              >
                Cancel
              </Button>
            </div>
          </div>
        )}

        {deposit.deposit_paid && manage && !editing && (
          <div className="flex flex-wrap gap-2">
            {(!d || d.status === "draft") && (
              <Button variant="outline" disabled={busy} onClick={startEditing}>
                {d ? "Edit deductions" : "Start disposition"}
              </Button>
            )}
            {d &&
              (d.status === "draft" || d.status === "failed") &&
              canFinalize && (
                <Button
                  disabled={busy}
                  onClick={() =>
                    void run(
                      () => api.finalizeDepositDisposition(d.id),
                      "Disposition finalized — refund in flight."
                    )
                  }
                >
                  {d.status === "failed" ? "Retry refund" : "Finalize & refund"}
                </Button>
              )}
          </div>
        )}
      </div>
    </Card>
  );
}
