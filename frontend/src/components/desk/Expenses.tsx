"use client";

// Money spent on a work order: purchases with their receipts. Snap the
// receipt, enter the total, done.

import { useMemo, useRef, useState } from "react";
import { Plus, Receipt } from "lucide-react";
import { toast } from "sonner";
import {
  desk,
  money,
  parseCents,
  type TicketExpense,
  type TicketFile,
} from "@/lib/servicedesk";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const field =
  "rounded-lg border border-line bg-surface px-2.5 py-1.5 text-[13px] text-fg outline-none focus:border-accent";

export function Expenses({
  ticketId,
  expenses,
  files,
  manage,
  onChange,
}: {
  ticketId: string;
  expenses: TicketExpense[];
  files: TicketFile[];
  manage: boolean;
  onChange: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [description, setDescription] = useState("");
  const [vendor, setVendor] = useState("");
  const [amount, setAmount] = useState("");
  const [billable, setBillable] = useState(false);
  const [reimburse, setReimburse] = useState(false);
  const [receipt, setReceipt] = useState<TicketFile | null>(null);
  const [busy, setBusy] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  const byId = useMemo(() => new Map(files.map((f) => [f.id, f])), [files]);
  const total = expenses.reduce((n, e) => n + e.amount_cents, 0);

  async function snap(f: File) {
    setBusy(true);
    try {
      setReceipt(await desk.upload(ticketId, f, "receipt"));
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Upload failed");
    } finally {
      setBusy(false);
    }
  }

  async function save() {
    const cents = parseCents(amount);
    if (!description.trim() || !cents) {
      toast.error("Say what was bought and how much.");
      return;
    }
    setBusy(true);
    try {
      await desk.addExpense(ticketId, {
        description: description.trim(),
        amount_cents: cents,
        vendor: vendor.trim() || undefined,
        receipt_document_ids: receipt ? [receipt.id] : [],
        billable_to_owner: billable,
        reimbursable: reimburse,
      });
      toast.success("Expense logged");
      setOpen(false);
      setDescription("");
      setVendor("");
      setAmount("");
      setReceipt(null);
      setBillable(false);
      setReimburse(false);
      onChange();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't save it");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Panel>
      <PanelHeader
        title="Expenses"
        description={
          expenses.length
            ? `${expenses.length} logged · ${money(total)}`
            : "Purchases and receipts."
        }
        action={
          manage &&
          !open && (
            <Button size="sm" variant="secondary" onClick={() => setOpen(true)}>
              <Plus />
              Expense
            </Button>
          )
        }
      />
      <div className="space-y-3 p-5">
        {open && (
          <div className="space-y-2 rounded-xl border border-line bg-fill/40 p-3">
            <input
              className={cn(field, "w-full")}
              placeholder="What was bought"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              autoFocus
            />
            <div className="flex gap-2">
              <input
                className={cn(field, "min-w-0 flex-1")}
                placeholder="Store or vendor"
                value={vendor}
                onChange={(e) => setVendor(e.target.value)}
              />
              <input
                className={cn(field, "w-28")}
                placeholder="$0.00"
                inputMode="decimal"
                value={amount}
                onChange={(e) => setAmount(e.target.value)}
                aria-label="Amount"
              />
            </div>
            <div className="flex flex-wrap items-center gap-3 text-xs text-fg-2">
              <input
                ref={input}
                type="file"
                accept="image/*,application/pdf"
                capture="environment"
                className="hidden"
                onChange={(e) => e.target.files?.[0] && snap(e.target.files[0])}
              />
              <Button
                type="button"
                size="sm"
                variant="ghost"
                onClick={() => input.current?.click()}
                disabled={busy}
              >
                <Receipt />
                {receipt ? receipt.filename : "Receipt"}
              </Button>
              <label className="flex items-center gap-1.5">
                <input
                  type="checkbox"
                  checked={billable}
                  onChange={(e) => setBillable(e.target.checked)}
                />
                Bill the owner
              </label>
              <label className="flex items-center gap-1.5">
                <input
                  type="checkbox"
                  checked={reimburse}
                  onChange={(e) => setReimburse(e.target.checked)}
                />
                Paid out of pocket
              </label>
            </div>
            <div className="flex justify-end gap-2">
              <Button size="sm" variant="ghost" onClick={() => setOpen(false)}>
                Cancel
              </Button>
              <Button size="sm" onClick={save} disabled={busy}>
                Save
              </Button>
            </div>
          </div>
        )}
        {expenses.length === 0 && !open && (
          <EmptyState
            icon={<Receipt />}
            title="Nothing spent yet"
            className="py-6"
          />
        )}
        <ul className="divide-y divide-line">
          {expenses.map((e) => {
            const r = e.receipt_document_ids
              .map((id) => byId.get(id))
              .find((f) => f?.url);
            return (
              <li key={e.id} className="flex items-center gap-3 py-2.5">
                <div className="min-w-0 flex-1">
                  <div className="truncate text-[13px] text-fg">
                    {e.description}
                  </div>
                  <div className="text-xs text-fg-3">
                    {[e.vendor, e.incurred_on].filter(Boolean).join(" · ")}
                  </div>
                </div>
                {e.billable_to_owner && <Badge tone="info">Owner</Badge>}
                {r?.url ? (
                  <a
                    href={r.url}
                    target="_blank"
                    rel="noreferrer"
                    className="text-xs font-medium text-accent"
                  >
                    Receipt
                  </a>
                ) : (
                  <Badge tone="warn">No receipt</Badge>
                )}
                <span className="figure w-20 text-right text-[13px] text-fg">
                  {money(e.amount_cents)}
                </span>
              </li>
            );
          })}
        </ul>
      </div>
    </Panel>
  );
}
