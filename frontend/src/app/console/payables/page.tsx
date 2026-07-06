"use client";

// Accounts payable: vendor bills drafted against an entity's books, routed
// through approval, and paid by ACH. Gated by `payable:read`; drafting and
// voiding need `payable:manage`, approving/rejecting/paying need
// `payable:approve`.

import { useMemo, useState } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { useQuery } from "@tanstack/react-query";

import { api } from "@/lib/api";
import type { VendorBill } from "@/lib/api";
import type { Counterparty } from "@/lib/types";
import { useAuth } from "@/lib/auth";
import {
  useCreatePayable,
  usePayableAction,
  usePayables,
  useProperties,
} from "@/lib/queries";
import {
  createPayableSchema,
  type CreatePayableInputForm,
} from "@/lib/schemas";
import { Badge, Button, Card, StatTile } from "@/components/ui";
import { Button as DialogButton } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";

const STATUSES = [
  "draft",
  "submitted",
  "approved",
  "processing",
  "paid",
  "failed",
  "void",
];

/** Tone for a vendor-bill status. */
function billTone(
  status: string
): "neutral" | "good" | "warn" | "bad" | "info" | "accent" {
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
      // draft, void
      return "neutral";
  }
}

function humanize(key: string): string {
  const s = key.replace(/_/g, " ");
  return s.charAt(0).toUpperCase() + s.slice(1);
}

export default function PayablesPage() {
  const { can } = useAuth();
  const manage = can("payable:manage");
  const approve = can("payable:approve");
  const [status, setStatus] = useState<string>("");

  const {
    data: bills,
    error,
    isLoading,
  } = usePayables(status ? { status } : {}, {
    // Follow in-flight payments to settlement.
    refetchInterval: (q) =>
      q.state.data?.some((b) => b.status === "processing") ? 4000 : false,
  });
  // Unfiltered list for the stat tiles (shares a cache entry when no filter).
  const { data: allBills } = usePayables({});
  const action = usePayableAction();

  const counts = useMemo(() => {
    const by = { submitted: 0, approved: 0, paid: 0 };
    for (const b of allBills ?? []) {
      if (b.status === "submitted") by.submitted += 1;
      else if (b.status === "approved") by.approved += 1;
      else if (b.status === "paid") by.paid += 1;
    }
    return by;
  }, [allBills]);

  const onReject = (bill: VendorBill) => {
    const reason = window.prompt(
      `Why is bill ${bill.bill_number} being rejected?`
    );
    if (reason === null) return;
    action.mutate({
      id: bill.id,
      action: "reject",
      reason: reason.trim() || undefined,
    });
  };

  if (!can("payable:read")) {
    return (
      <Card className="p-6">
        <p className="text-ink-2">
          You don&apos;t have access to payables. Ask an admin for the{" "}
          <span className="font-mono">payable:read</span> permission.
        </p>
      </Card>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            Payables
          </h1>
          <p className="text-ink-3">
            Vendor bills from draft through approval to paid — each one accrued
            and settled on the entity&apos;s books.
          </p>
        </div>
        <div className="flex items-end gap-3">
          <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
            Status
            <select
              value={status}
              onChange={(e) => setStatus(e.target.value)}
              className="rounded-xl border border-line bg-surface px-3 py-2 text-sm font-normal text-ink"
            >
              <option value="">All</option>
              {STATUSES.map((s) => (
                <option key={s} value={s}>
                  {humanize(s)}
                </option>
              ))}
            </select>
          </label>
          {manage && <NewBillDialog />}
        </div>
      </div>

      <div className="grid gap-4 sm:grid-cols-3">
        <StatTile label="Awaiting approval" value={String(counts.submitted)} />
        <StatTile label="Approved, unpaid" value={String(counts.approved)} />
        <StatTile label="Paid" value={String(counts.paid)} />
      </div>

      {error && <p className="text-bad">{error.message}</p>}

      <Card className="overflow-hidden">
        <div className="grid grid-cols-[1.8fr_1fr_1fr_.7fr_.8fr_1.2fr] gap-4 border-b border-line px-5 py-3 text-xs font-bold uppercase tracking-wide text-ink-3">
          <span>Bill</span>
          <span>Vendor</span>
          <span>Entity</span>
          <span className="text-right">Amount</span>
          <span>Status</span>
          <span className="text-right">Actions</span>
        </div>
        <div className="divide-y divide-line">
          {bills?.map((b) => (
            <div
              key={b.id}
              className="grid grid-cols-[1.8fr_1fr_1fr_.7fr_.8fr_1.2fr] items-center gap-4 px-5 py-3.5"
            >
              <div className="min-w-0">
                <div className="truncate font-semibold">
                  <span className="font-mono text-sm text-ink-3">
                    {b.bill_number}
                  </span>{" "}
                  {b.memo}
                </div>
                {b.status === "draft" && b.rejected_reason && (
                  <div className="mt-0.5 truncate text-xs text-warn">
                    Rejected: {b.rejected_reason}
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
              <span className="truncate text-sm text-ink-2">
                {b.vendor_name ?? "—"}
              </span>
              <span className="truncate text-sm text-ink-2">
                {b.entity_name ?? "—"}
              </span>
              <span className="text-right font-semibold">{b.amount_label}</span>
              <span>
                <Badge tone={billTone(b.status)}>{b.status}</Badge>
              </span>
              <span className="flex flex-wrap justify-end gap-2">
                {b.status === "draft" && manage && (
                  <>
                    <Button
                      className="px-3 py-1.5 text-xs"
                      disabled={action.isPending}
                      onClick={() =>
                        action.mutate({ id: b.id, action: "submit" })
                      }
                    >
                      Submit
                    </Button>
                    <Button
                      variant="outline"
                      className="px-3 py-1.5 text-xs"
                      disabled={action.isPending}
                      onClick={() =>
                        action.mutate({ id: b.id, action: "void" })
                      }
                    >
                      Void
                    </Button>
                  </>
                )}
                {b.status === "submitted" && (
                  <>
                    {approve && (
                      <>
                        <Button
                          className="px-3 py-1.5 text-xs"
                          disabled={action.isPending}
                          onClick={() =>
                            action.mutate({ id: b.id, action: "approve" })
                          }
                        >
                          Approve
                        </Button>
                        <Button
                          variant="outline"
                          className="px-3 py-1.5 text-xs"
                          disabled={action.isPending}
                          onClick={() => onReject(b)}
                        >
                          Reject
                        </Button>
                      </>
                    )}
                    {manage && (
                      <Button
                        variant="outline"
                        className="px-3 py-1.5 text-xs"
                        disabled={action.isPending}
                        onClick={() =>
                          action.mutate({ id: b.id, action: "void" })
                        }
                      >
                        Void
                      </Button>
                    )}
                  </>
                )}
                {b.status === "approved" && approve && (
                  <Button
                    className="px-3 py-1.5 text-xs"
                    disabled={action.isPending}
                    onClick={() => action.mutate({ id: b.id, action: "pay" })}
                  >
                    Pay
                  </Button>
                )}
                {b.status === "failed" && approve && (
                  <Button
                    className="px-3 py-1.5 text-xs"
                    disabled={action.isPending}
                    onClick={() => action.mutate({ id: b.id, action: "pay" })}
                  >
                    Retry pay
                  </Button>
                )}
              </span>
            </div>
          ))}
          {isLoading && (
            <div className="px-5 py-10 text-center text-ink-3">Loading…</div>
          )}
          {bills && bills.length === 0 && (
            <div className="px-5 py-10 text-center text-ink-3">
              No bills{status ? " in this status" : " yet — draft one above"}.
            </div>
          )}
        </div>
      </Card>
    </div>
  );
}

/** Dialog to draft a new vendor bill. */
function NewBillDialog() {
  const [open, setOpen] = useState(false);
  const create = useCreatePayable();
  const { data: properties } = useProperties();
  const { data: vendors } = useQuery<Counterparty[], Error>({
    queryKey: ["entities"],
    queryFn: () => api.entities(),
    enabled: open,
  });

  const {
    register,
    handleSubmit,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<CreatePayableInputForm>({
    resolver: zodResolver(createPayableSchema),
    defaultValues: {
      counterparty_id: "",
      property_id: "",
      maintenance_ticket_id: "",
      memo: "",
      due_date: "",
    },
  });

  const onSubmit = handleSubmit(async (values) => {
    await create.mutateAsync(
      {
        counterparty_id: values.counterparty_id,
        property_id: values.property_id || undefined,
        maintenance_ticket_id: values.maintenance_ticket_id || undefined,
        memo: values.memo,
        amount_cents: Math.round(values.amount * 100),
        due_date: values.due_date || undefined,
      },
      {
        onSuccess: () => {
          reset();
          setOpen(false);
        },
      }
    );
  });

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <DialogButton>New bill</DialogButton>
      </DialogTrigger>
      <DialogContent>
        <form onSubmit={onSubmit}>
          <DialogHeader>
            <DialogTitle>New bill</DialogTitle>
            <DialogDescription>
              Draft a vendor bill — it goes through approval before it&apos;s
              paid.
            </DialogDescription>
          </DialogHeader>
          <div className="my-5 space-y-4">
            <div className="space-y-1.5">
              <Label>Vendor</Label>
              <select
                className="flex h-10 w-full rounded-xl border border-line bg-surface-2 px-3 text-sm outline-none focus:border-accent"
                aria-invalid={!!errors.counterparty_id}
                {...register("counterparty_id")}
              >
                <option value="">— Select —</option>
                {vendors?.map((v) => (
                  <option key={v.id} value={v.id}>
                    {v.name} ({v.kind})
                  </option>
                ))}
              </select>
              {errors.counterparty_id && (
                <p className="text-sm text-bad" role="alert">
                  {errors.counterparty_id.message}
                </p>
              )}
            </div>
            <div className="space-y-1.5">
              <Label>Property (optional)</Label>
              <select
                className="flex h-10 w-full rounded-xl border border-line bg-surface-2 px-3 text-sm outline-none focus:border-accent"
                {...register("property_id")}
              >
                <option value="">— None —</option>
                {properties?.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </select>
              <p className="text-xs text-ink-3">
                The paying entity&apos;s (LLC&apos;s) books are derived from the
                property.
              </p>
            </div>
            <div className="space-y-1.5">
              <Label>Memo</Label>
              <Input
                placeholder="e.g. HVAC compressor replacement"
                aria-invalid={!!errors.memo}
                {...register("memo")}
              />
              {errors.memo && (
                <p className="text-sm text-bad" role="alert">
                  {errors.memo.message}
                </p>
              )}
            </div>
            <div className="space-y-1.5">
              <Label>Amount ($)</Label>
              <Input
                type="number"
                step="0.01"
                placeholder="0.00"
                aria-invalid={!!errors.amount}
                {...register("amount")}
              />
              {errors.amount && (
                <p className="text-sm text-bad" role="alert">
                  {errors.amount.message}
                </p>
              )}
            </div>
            <div className="space-y-1.5">
              <Label>Due date (optional)</Label>
              <Input type="date" {...register("due_date")} />
            </div>
            <div className="space-y-1.5">
              <Label>Maintenance ticket id (optional)</Label>
              <Input
                placeholder="Prefill from a completed work order"
                {...register("maintenance_ticket_id")}
              />
              <p className="text-xs text-ink-3">
                Links the bill to a completed work order.
              </p>
            </div>
          </div>
          <DialogFooter>
            <DialogButton
              type="button"
              variant="outline"
              onClick={() => setOpen(false)}
            >
              Cancel
            </DialogButton>
            <DialogButton type="submit" disabled={isSubmitting}>
              {isSubmitting ? "Drafting…" : "Draft bill"}
            </DialogButton>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
