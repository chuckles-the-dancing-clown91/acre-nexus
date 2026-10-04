"use client";

// Payables: vendor bills drafted against an entity's books, sent for
// approval, and paid by ACH. Reading needs `payable:read`; drafting and
// voiding need `payable:manage`; approving, rejecting and paying need
// `payable:approve`.

import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Ban,
  Check,
  Lock,
  Plus,
  ReceiptText,
  RotateCcw,
  Send,
  Undo2,
  Wallet,
} from "lucide-react";
import { toast } from "sonner";
import { api, type VendorBill } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { queryKeys, useProperties } from "@/lib/queries";
import { createPayableSchema } from "@/lib/schemas";
import { usd } from "@/lib/format";
import { errMsg, humanize, useReady } from "@/lib/money-extra";
import { Badge, type Tone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Stat } from "@/components/ui/data-table";
import { Field, fieldClass, Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const STATUSES = [
  "draft",
  "submitted",
  "approved",
  "processing",
  "paid",
  "failed",
  "void",
];

function billTone(status: string): Tone {
  switch (status) {
    case "submitted":
      return "info";
    case "approved":
      return "accent";
    case "processing":
      return "warn";
    case "paid":
      return "good";
    case "failed":
      return "bad";
    default:
      return "neutral";
  }
}

type Action = "submit" | "approve" | "reject" | "void" | "pay";

const DONE: Record<Action, string> = {
  submit: "sent for approval",
  approve: "approved and accrued to the ledger",
  reject: "sent back to draft",
  void: "voided",
  pay: "paying. It posts to the ledger when it settles",
};

export default function PayablesPage() {
  const { can } = useAuth();
  const ready = useReady("payable:read");
  const allowed = useReady();
  const manage = can("payable:manage");
  const approve = can("payable:approve");
  const qc = useQueryClient();
  const [status, setStatus] = useState("");
  const [adding, setAdding] = useState(false);
  const [rejecting, setRejecting] = useState<VendorBill | null>(null);

  const params = useMemo(() => (status ? { status } : {}), [status]);
  const bills = useQuery({
    queryKey: queryKeys.payables(params),
    queryFn: () => api.payables(params),
    enabled: ready,
    // Follow in-flight payments to settlement.
    refetchInterval: (q) =>
      q.state.data?.some((b) => b.status === "processing") ? 4000 : false,
  });
  // Unfiltered, for the counts (shares the cache when no filter is set).
  const all = useQuery({
    queryKey: queryKeys.payables({}),
    queryFn: () => api.payables({}),
    enabled: ready,
  });
  const counts = useMemo(() => {
    const by = { submitted: 0, approved: 0, paid: 0, owed: 0 };
    for (const b of all.data ?? []) {
      if (b.status === "submitted") by.submitted += 1;
      else if (b.status === "approved") {
        by.approved += 1;
        by.owed += b.amount_cents;
      } else if (b.status === "paid") by.paid += 1;
    }
    return by;
  }, [all.data]);

  const act = useMutation({
    mutationFn: ({
      id,
      action,
      reason,
    }: {
      id: string;
      action: Action;
      reason?: string;
    }) => {
      switch (action) {
        case "submit":
          return api.submitPayable(id);
        case "approve":
          return api.approvePayable(id);
        case "reject":
          return api.rejectPayable(id, reason);
        case "void":
          return api.voidPayable(id);
        case "pay":
          return api.payPayable(id);
      }
    },
    onSuccess: (bill, { action }) => {
      void qc.invalidateQueries({ queryKey: ["payables"] });
      toast.success(`Bill ${bill.bill_number} ${DONE[action]}`);
    },
    onError: (e) => toast.error(errMsg(e, "Couldn't update the bill")),
  });

  if (allowed && !ready)
    return (
      <div className="space-y-6">
        <PageHeader eyebrow="Money" title="Payables" />
        <Panel>
          <EmptyState
            icon={<Lock />}
            title="You don't have access to payables"
            description="Ask an admin for the payable:read permission."
          />
        </Panel>
      </div>
    );

  const run = (b: VendorBill, action: Action) =>
    act.mutate({ id: b.id, action });

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Money"
        title="Payables"
        description="Vendor bills from draft through approval to paid, each accrued and settled on the entity's books."
        actions={
          manage && (
            <Button onClick={() => setAdding(true)}>
              <Plus />
              New bill
            </Button>
          )
        }
      />

      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        <Stat
          label="Waiting approval"
          value={counts.submitted}
          tone={counts.submitted ? "warn" : undefined}
        />
        <Stat label="Approved, not paid" value={counts.approved} />
        <Stat label="Owed on approved" value={usd(counts.owed)} />
        <Stat label="Paid" value={counts.paid} tone="good" />
      </div>

      <Panel className="overflow-hidden">
        <div className="flex flex-wrap items-center justify-between gap-3 border-b border-line p-3">
          <div className="text-[13px] text-fg-3">
            {bills.data
              ? `${bills.data.length} ${bills.data.length === 1 ? "bill" : "bills"}`
              : "Bills"}
          </div>
          <select
            aria-label="Status"
            className={fieldClass}
            value={status}
            onChange={(e) => setStatus(e.target.value)}
          >
            <option value="">Every status</option>
            {STATUSES.map((s) => (
              <option key={s} value={s}>
                {humanize(s)}
              </option>
            ))}
          </select>
        </div>

        {bills.error && (
          <p className="p-4 text-[13px] text-bad">
            Couldn&apos;t load bills: {bills.error.message}
          </p>
        )}
        {bills.isLoading && (
          <div className="space-y-2 p-3">
            {Array.from({ length: 5 }, (_, i) => (
              <Skeleton key={i} className="h-14" />
            ))}
          </div>
        )}
        {bills.data && bills.data.length === 0 && (
          <EmptyState
            icon={<ReceiptText />}
            title={status ? "No bills in this status" : "No bills yet"}
            description={
              !status && manage ? "Draft one with New bill." : undefined
            }
          />
        )}

        <ul className="divide-y divide-line">
          {bills.data?.map((b) => (
            <li
              key={b.id}
              className="flex flex-col gap-3 px-4 py-3 md:flex-row md:items-center"
            >
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="font-mono text-xs text-fg-3">
                    {b.bill_number}
                  </span>
                  <span className="truncate text-[14px] font-medium text-fg">
                    {b.memo}
                  </span>
                </div>
                <div className="truncate text-xs text-fg-3">
                  {b.vendor_name ?? "No vendor"} ·{" "}
                  {b.entity_name ?? "No entity"}
                  {b.due_date ? ` · due ${b.due_date}` : ""}
                </div>
                {b.status === "draft" && b.rejected_reason && (
                  <div className="mt-0.5 truncate text-xs text-warn">
                    Sent back: {b.rejected_reason}
                  </div>
                )}
                {b.status === "failed" && b.failure_reason && (
                  <div
                    className="mt-0.5 truncate text-xs text-bad"
                    title={b.failure_reason}
                  >
                    {b.failure_reason}
                  </div>
                )}
              </div>
              <div className="flex flex-wrap items-center gap-3 md:justify-end">
                <span className="figure text-[14px] font-semibold text-fg">
                  {b.amount_label}
                </span>
                <Badge tone={billTone(b.status)}>{b.status}</Badge>
                <div className="flex flex-wrap gap-1.5">
                  {b.status === "draft" && manage && (
                    <>
                      <Button
                        size="sm"
                        disabled={act.isPending}
                        onClick={() => run(b, "submit")}
                      >
                        <Send />
                        Submit
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        disabled={act.isPending}
                        onClick={() => run(b, "void")}
                      >
                        <Ban />
                        Void
                      </Button>
                    </>
                  )}
                  {b.status === "submitted" && approve && (
                    <>
                      <Button
                        size="sm"
                        disabled={act.isPending}
                        onClick={() => run(b, "approve")}
                      >
                        <Check />
                        Approve
                      </Button>
                      <Button
                        size="sm"
                        variant="secondary"
                        disabled={act.isPending}
                        onClick={() => setRejecting(b)}
                      >
                        <Undo2 />
                        Reject
                      </Button>
                    </>
                  )}
                  {b.status === "submitted" && manage && (
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={act.isPending}
                      onClick={() => run(b, "void")}
                    >
                      <Ban />
                      Void
                    </Button>
                  )}
                  {b.status === "approved" && approve && (
                    <Button
                      size="sm"
                      disabled={act.isPending}
                      onClick={() => run(b, "pay")}
                    >
                      <Wallet />
                      Pay
                    </Button>
                  )}
                  {b.status === "failed" && approve && (
                    <Button
                      size="sm"
                      disabled={act.isPending}
                      onClick={() => run(b, "pay")}
                    >
                      <RotateCcw />
                      Retry pay
                    </Button>
                  )}
                </div>
              </div>
            </li>
          ))}
        </ul>
      </Panel>

      {adding && <NewBillDialog onClose={() => setAdding(false)} />}
      {rejecting && (
        <RejectDialog
          bill={rejecting}
          busy={act.isPending}
          onClose={() => setRejecting(null)}
          onReject={(reason) =>
            act.mutate(
              { id: rejecting.id, action: "reject", reason },
              { onSuccess: () => setRejecting(null) }
            )
          }
        />
      )}
    </div>
  );
}

function RejectDialog({
  bill,
  busy,
  onClose,
  onReject,
}: {
  bill: VendorBill;
  busy: boolean;
  onClose: () => void;
  onReject: (reason?: string) => void;
}) {
  const [reason, setReason] = useState("");
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md">
        <DialogTitle className="text-[17px] font-semibold">
          Reject bill {bill.bill_number}
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          It goes back to draft with your reason, so it can be fixed and sent
          again.
        </DialogDescription>
        <textarea
          aria-label="Reason"
          className={cn(fieldClass, "mt-4 min-h-[72px] w-full")}
          placeholder="Why (optional)"
          value={reason}
          onChange={(e) => setReason(e.target.value)}
        />
        <div className="mt-4 flex justify-end gap-2">
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button
            variant="danger"
            loading={busy}
            onClick={() => onReject(reason.trim() || undefined)}
          >
            <Undo2 />
            Reject
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

function NewBillDialog({ onClose }: { onClose: () => void }) {
  const qc = useQueryClient();
  const scoped = useReady();
  const properties = useProperties({ enabled: scoped });
  const vendors = useQuery({
    queryKey: ["entities"],
    queryFn: () => api.entities(),
    enabled: scoped,
  });
  const [form, setForm] = useState({
    counterparty_id: "",
    property_id: "",
    maintenance_ticket_id: "",
    memo: "",
    amount: "",
    due_date: "",
  });
  const [errors, setErrors] = useState<Record<string, string>>({});
  const set = (k: keyof typeof form) => (v: string) =>
    setForm((f) => ({ ...f, [k]: v }));

  const create = useMutation({
    mutationFn: api.createPayable,
    onSuccess: (bill) => {
      void qc.invalidateQueries({ queryKey: ["payables"] });
      toast.success(`Bill ${bill.bill_number} drafted`);
      onClose();
    },
    onError: (e) => toast.error(errMsg(e, "Couldn't create the bill")),
  });

  function submit(e: React.FormEvent) {
    e.preventDefault();
    const parsed = createPayableSchema.safeParse(form);
    if (!parsed.success) {
      const next: Record<string, string> = {};
      for (const issue of parsed.error.issues) {
        const k = String(issue.path[0]);
        next[k] ??= issue.message;
      }
      setErrors(next);
      return;
    }
    setErrors({});
    const v = parsed.data;
    create.mutate({
      counterparty_id: v.counterparty_id,
      property_id: v.property_id || undefined,
      maintenance_ticket_id: v.maintenance_ticket_id || undefined,
      memo: v.memo,
      amount_cents: Math.round(v.amount * 100),
      due_date: v.due_date || undefined,
    });
  }

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-h-[90vh] max-w-lg overflow-y-auto">
        <form onSubmit={submit}>
          <DialogTitle className="text-[17px] font-semibold">
            New bill
          </DialogTitle>
          <DialogDescription className="mt-1 text-[13px] text-fg-3">
            Draft a vendor bill. It goes through approval before it&apos;s paid.
          </DialogDescription>
          <div className="mt-5 space-y-4">
            <Field label="Vendor" error={errors.counterparty_id}>
              {(p) => (
                <select
                  {...p}
                  className={cn(fieldClass, "h-11 w-full")}
                  value={form.counterparty_id}
                  onChange={(e) => set("counterparty_id")(e.target.value)}
                >
                  <option value="">Pick the vendor</option>
                  {vendors.data?.map((v) => (
                    <option key={v.id} value={v.id}>
                      {v.name} ({v.kind})
                    </option>
                  ))}
                </select>
              )}
            </Field>
            <Field
              label="Property (optional)"
              hint="The paying entity's books come from the property."
            >
              {(p) => (
                <select
                  {...p}
                  className={cn(fieldClass, "h-11 w-full")}
                  value={form.property_id}
                  onChange={(e) => set("property_id")(e.target.value)}
                >
                  <option value="">None</option>
                  {properties.data?.map((x) => (
                    <option key={x.id} value={x.id}>
                      {x.name}
                    </option>
                  ))}
                </select>
              )}
            </Field>
            <Field label="Memo" error={errors.memo}>
              {(p) => (
                <Input
                  {...p}
                  placeholder="HVAC compressor replacement"
                  value={form.memo}
                  onChange={(e) => set("memo")(e.target.value)}
                />
              )}
            </Field>
            <div className="grid gap-4 sm:grid-cols-2">
              <Field label="Amount ($)" error={errors.amount}>
                {(p) => (
                  <Input
                    {...p}
                    type="number"
                    step="0.01"
                    inputMode="decimal"
                    placeholder="0.00"
                    value={form.amount}
                    onChange={(e) => set("amount")(e.target.value)}
                  />
                )}
              </Field>
              <Field label="Due date (optional)">
                {(p) => (
                  <Input
                    {...p}
                    type="date"
                    value={form.due_date}
                    onChange={(e) => set("due_date")(e.target.value)}
                  />
                )}
              </Field>
            </div>
            <Field
              label="Work order ID (optional)"
              hint="Links the bill to a finished work order."
            >
              {(p) => (
                <Input
                  {...p}
                  placeholder="Paste the work order ID"
                  value={form.maintenance_ticket_id}
                  onChange={(e) => set("maintenance_ticket_id")(e.target.value)}
                />
              )}
            </Field>
          </div>
          <div className="mt-6 flex justify-end gap-2">
            <Button type="button" variant="ghost" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" loading={create.isPending}>
              <Plus />
              Draft bill
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}
