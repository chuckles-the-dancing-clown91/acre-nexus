"use client";

// Leases / rent roll: every tenancy with its rent, status, and balance. Click a
// row to open a lease detail dialog showing the payment ledger and (gated by
// `lease:manage`) a "Record payment" form. Gated by `lease:read`.

import * as React from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import {
  Banknote,
  CalendarClock,
  CircleDollarSign,
  Lock,
  Receipt,
  ScrollText,
} from "lucide-react";

import { useAuth } from "@/lib/auth";
import { useLeases, useLease, useProperties, useRecordPayment } from "@/lib/queries";
import type { Lease } from "@/lib/types";
import { currencyFromCents, formatDate } from "@/lib/format";
import { cn } from "@/lib/utils";

import { PageHeader, StatCard, EmptyState } from "@/components/ui/page";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { DataTable, type ColumnDef } from "@/components/ui/data-table";
import { Button } from "@/components/ui/button";
import { Badge, statusTone } from "@/components/ui";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Field, Input, SelectField } from "@/components/ui/form-field";

const STATUS_OPTIONS = [
  { value: "all", label: "All statuses" },
  { value: "active", label: "Active" },
  { value: "upcoming", label: "Upcoming" },
  { value: "notice", label: "Notice" },
  { value: "expired", label: "Expired" },
  { value: "ended", label: "Ended" },
];

/** Tone for a lease's payment standing. */
function paymentTone(status: string): "neutral" | "good" | "warn" | "bad" {
  const s = status.toLowerCase();
  if (s === "current" || s === "paid") return "good";
  if (s === "partial") return "warn";
  if (s === "late" || s === "overdue") return "bad";
  return "neutral";
}

export default function LeasesPage() {
  const { can } = useAuth();
  const canManage = can("lease:manage");

  const [status, setStatus] = React.useState<string>("all");
  const [activeLease, setActiveLease] = React.useState<Lease | null>(null);

  const leasesQuery = useLeases(status === "all" ? undefined : { status });
  const properties = useProperties();

  const leases = leasesQuery.data ?? [];

  const propName = React.useMemo(() => {
    const m = new Map((properties.data ?? []).map((p) => [p.id, p.name]));
    return (id: string) => m.get(id) ?? "—";
  }, [properties.data]);

  if (!can("lease:read")) {
    return (
      <div className="space-y-6">
        <PageHeader
          eyebrow="Rentals"
          title="Leases"
          description="The rent roll across your portfolio."
        />
        <EmptyState
          icon={Lock}
          title="No access to rentals"
          description="Ask an admin for the lease:read permission to view the rent roll."
        />
      </div>
    );
  }

  // Portfolio-level rent-roll stats (computed from the loaded rows).
  const monthlyRentCents = leases.reduce((sum, l) => sum + l.rent_cents, 0);
  const outstandingCents = leases.reduce((sum, l) => sum + l.balance_cents, 0);
  const activeCount = leases.filter(
    (l) => l.status.toLowerCase() === "active"
  ).length;

  const columns: ColumnDef<Lease>[] = [
    {
      accessorKey: "tenant_name",
      header: "Tenant",
      cell: ({ row }) => {
        const l = row.original;
        return (
          <div className="min-w-0">
            <div className="truncate font-medium text-ink">{l.tenant_name}</div>
            {l.tenant_email && (
              <div className="truncate text-xs text-ink-3">
                {l.tenant_email}
              </div>
            )}
          </div>
        );
      },
    },
    {
      id: "property",
      header: "Property / Unit",
      accessorFn: (l) => propName(l.property_id),
      cell: ({ row }) => {
        const l = row.original;
        return (
          <div className="min-w-0">
            <div className="truncate text-ink-2">{propName(l.property_id)}</div>
            {l.unit_id && (
              <div className="truncate font-mono text-xs text-ink-3">
                Unit · {l.unit_id.slice(0, 8)}
              </div>
            )}
          </div>
        );
      },
    },
    {
      accessorKey: "rent_cents",
      header: "Rent",
      cell: ({ row }) => (
        <span data-numeric className="font-medium text-ink">
          {row.original.rent_label}
        </span>
      ),
    },
    {
      accessorKey: "status",
      header: "Status",
      cell: ({ row }) => (
        <Badge tone={statusTone(row.original.status)}>
          {row.original.status}
        </Badge>
      ),
    },
    {
      accessorKey: "payment_status",
      header: "Payment",
      cell: ({ row }) => (
        <Badge tone={paymentTone(row.original.payment_status)}>
          {row.original.payment_status}
        </Badge>
      ),
    },
    {
      accessorKey: "balance_cents",
      header: "Balance",
      cell: ({ row }) => {
        const bal = row.original.balance_cents;
        return (
          <span
            data-numeric
            className={cn(
              "font-medium",
              bal > 0 ? "text-bad" : "text-ink-3"
            )}
          >
            {currencyFromCents(bal)}
          </span>
        );
      },
    },
  ];

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Rentals"
        title="Leases"
        description="The rent roll across your portfolio — tenancies, rent, and payment standing."
        actions={
          <Select value={status} onValueChange={setStatus}>
            <SelectTrigger className="w-[180px]" aria-label="Filter by status">
              <SelectValue placeholder="All statuses" />
            </SelectTrigger>
            <SelectContent>
              {STATUS_OPTIONS.map((o) => (
                <SelectItem key={o.value} value={o.value}>
                  {o.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        }
      />

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
        {leasesQuery.isLoading ? (
          Array.from({ length: 3 }).map((_, i) => (
            <div key={i} className="skeleton h-[104px] rounded-xl" />
          ))
        ) : (
          <>
            <StatCard
              label="Scheduled rent"
              value={currencyFromCents(monthlyRentCents)}
              sub="Across loaded leases"
              icon={Banknote}
              tone="good"
            />
            <StatCard
              label="Active leases"
              value={activeCount}
              sub={`${leases.length} total`}
              icon={ScrollText}
            />
            <StatCard
              label="Outstanding balance"
              value={currencyFromCents(outstandingCents)}
              sub="Unpaid across tenancies"
              icon={CircleDollarSign}
              tone={outstandingCents > 0 ? "bad" : "neutral"}
            />
          </>
        )}
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Rent roll</CardTitle>
        </CardHeader>
        <CardContent>
          <DataTable
            columns={columns}
            data={leases}
            isLoading={leasesQuery.isLoading}
            onRowClick={(l) => setActiveLease(l)}
            searchPlaceholder="Search tenants…"
            emptyState={
              <EmptyState
                className="border-0 bg-transparent"
                icon={Receipt}
                title="No leases"
                description={
                  status === "all"
                    ? "No tenancies have been created yet."
                    : "No leases match this status filter."
                }
              />
            }
          />
        </CardContent>
      </Card>

      <LeaseDetailDialog
        lease={activeLease}
        propName={activeLease ? propName(activeLease.property_id) : ""}
        canManage={canManage}
        onClose={() => setActiveLease(null)}
      />
    </div>
  );
}

// ---- Lease detail dialog ----------------------------------------------------

function LeaseDetailDialog({
  lease,
  propName,
  canManage,
  onClose,
}: {
  lease: Lease | null;
  propName: string;
  canManage: boolean;
  onClose: () => void;
}) {
  const open = lease != null;
  const detail = useLease(lease?.id ?? "", { enabled: open });

  const d = detail.data;
  const payments = d?.payments ?? [];

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>{lease?.tenant_name ?? "Lease"}</DialogTitle>
          <p className="text-sm text-ink-3">
            {propName}
            {lease?.tenant_email ? ` · ${lease.tenant_email}` : ""}
          </p>
        </DialogHeader>

        {lease && (
          <div className="space-y-5">
            {/* Summary chips */}
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
              <SummaryStat label="Rent" value={lease.rent_label} />
              <SummaryStat
                label="Deposit"
                value={lease.deposit_label ?? "—"}
              />
              <SummaryStat
                label="Balance"
                value={currencyFromCents(lease.balance_cents)}
                tone={lease.balance_cents > 0 ? "bad" : "neutral"}
              />
              <SummaryStat
                label="Term"
                value={
                  formatDate(lease.start_date) +
                  (lease.end_date ? ` → ${formatDate(lease.end_date)}` : " →")
                }
              />
            </div>

            <div className="flex flex-wrap items-center gap-2">
              <Badge tone={statusTone(lease.status)}>{lease.status}</Badge>
              <Badge tone={paymentTone(lease.payment_status)}>
                {lease.payment_status}
              </Badge>
            </div>

            {/* Payments ledger */}
            <div className="space-y-2">
              <h4 className="text-xs font-semibold uppercase tracking-wide text-ink-3">
                Payments
              </h4>
              {detail.isLoading ? (
                <div className="space-y-2">
                  {Array.from({ length: 3 }).map((_, i) => (
                    <div key={i} className="skeleton h-10 rounded-lg" />
                  ))}
                </div>
              ) : payments.length === 0 ? (
                <div className="rounded-lg border border-dashed border-line-2 px-4 py-6 text-center text-sm text-ink-3">
                  No payments recorded yet.
                </div>
              ) : (
                <div className="overflow-hidden rounded-lg border border-line">
                  <table className="w-full text-sm">
                    <thead>
                      <tr className="border-b border-line text-left text-xs font-semibold uppercase tracking-wide text-ink-3">
                        <th className="px-3 py-2 font-semibold">Due</th>
                        <th className="px-3 py-2 text-right font-semibold">
                          Amount
                        </th>
                        <th className="px-3 py-2 font-semibold">Status</th>
                        <th className="px-3 py-2 font-semibold">Paid</th>
                      </tr>
                    </thead>
                    <tbody>
                      {payments.map((p) => (
                        <tr
                          key={p.id}
                          className="border-b border-line last:border-0"
                        >
                          <td
                            data-numeric
                            className="px-3 py-2 text-ink-2"
                          >
                            {formatDate(p.due_date)}
                          </td>
                          <td
                            data-numeric
                            className="px-3 py-2 text-right font-medium text-ink"
                          >
                            {p.amount_label}
                          </td>
                          <td className="px-3 py-2">
                            <Badge tone={paymentTone(p.status)}>
                              {p.status}
                            </Badge>
                          </td>
                          <td
                            data-numeric
                            className="px-3 py-2 text-ink-3"
                          >
                            {p.paid_date ? formatDate(p.paid_date) : "—"}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </div>

            {/* Record payment */}
            {canManage && (
              <RecordPaymentForm
                leaseId={lease.id}
                defaultAmountCents={lease.rent_cents}
              />
            )}
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}

function SummaryStat({
  label,
  value,
  tone = "neutral",
}: {
  label: string;
  value: React.ReactNode;
  tone?: "neutral" | "bad";
}) {
  return (
    <div className="rounded-lg border border-line bg-surface-2/40 px-3 py-2">
      <div className="text-xs font-semibold uppercase tracking-wide text-ink-3">
        {label}
      </div>
      <div
        data-numeric
        className={cn(
          "mt-0.5 text-sm font-medium",
          tone === "bad" ? "text-bad" : "text-ink"
        )}
      >
        {value}
      </div>
    </div>
  );
}

// ---- Record payment form ----------------------------------------------------

const paymentSchema = z.object({
  due_date: z.string().min(1, "Required"),
  amount: z.coerce.number().positive("Must be greater than 0"),
  status: z.string().min(1),
  method: z.string().optional(),
  paid_date: z.string().optional(),
});

type PaymentForm = z.input<typeof paymentSchema>;

function RecordPaymentForm({
  leaseId,
  defaultAmountCents,
}: {
  leaseId: string;
  defaultAmountCents: number;
}) {
  const record = useRecordPayment(leaseId);
  const today = new Date().toISOString().slice(0, 10);

  const {
    register,
    handleSubmit,
    reset,
    formState: { errors },
  } = useForm<PaymentForm>({
    resolver: zodResolver(paymentSchema),
    defaultValues: {
      due_date: today,
      amount: defaultAmountCents / 100,
      status: "paid",
      method: "",
      paid_date: today,
    },
  });

  const onSubmit = handleSubmit((values) => {
    record.mutate(
      {
        due_date: values.due_date,
        amount_cents: Math.round(Number(values.amount) * 100),
        status: values.status,
        method: values.method || undefined,
        paid_date: values.paid_date || undefined,
      },
      {
        onSuccess: () =>
          reset({
            due_date: today,
            amount: defaultAmountCents / 100,
            status: "paid",
            method: "",
            paid_date: today,
          }),
      }
    );
  });

  return (
    <Card className="bg-surface-2/30">
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <CalendarClock className="h-4 w-4 text-ink-3" />
          Record payment
        </CardTitle>
        <CardDescription>
          Log a rent payment against this lease.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <form onSubmit={onSubmit} className="space-y-4">
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <Field label="Due date" htmlFor="due_date" error={errors.due_date?.message}>
              <Input id="due_date" type="date" {...register("due_date")} />
            </Field>
            <Field label="Amount (USD)" htmlFor="amount" error={errors.amount?.message}>
              <Input
                id="amount"
                type="number"
                step="0.01"
                min="0"
                {...register("amount")}
              />
            </Field>
            <SelectField label="Status" id="status" {...register("status")}>
              <option value="paid">Paid</option>
              <option value="partial">Partial</option>
              <option value="late">Late</option>
              <option value="pending">Pending</option>
            </SelectField>
            <Field label="Paid date" htmlFor="paid_date" error={errors.paid_date?.message}>
              <Input id="paid_date" type="date" {...register("paid_date")} />
            </Field>
            <Field
              label="Method"
              htmlFor="method"
              className="sm:col-span-2"
            >
              <Input
                id="method"
                placeholder="e.g. ACH, check, card"
                {...register("method")}
              />
            </Field>
          </div>
          <div className="flex justify-end">
            <Button type="submit" disabled={record.isPending}>
              {record.isPending ? "Recording…" : "Record payment"}
            </Button>
          </div>
        </form>
      </CardContent>
    </Card>
  );
}
