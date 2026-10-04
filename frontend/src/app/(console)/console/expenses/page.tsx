"use client";

// Expenses and mileage: everything the business spends (receipts, trips,
// materials), who needs paying back, and what can be billed to owners.
// Reading needs `expense:read`; adding, editing and paying back need
// `expense:manage`.

import { useMemo, useState } from "react";
import Link from "next/link";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Car,
  Check,
  FileText,
  Lock,
  Paperclip,
  Pencil,
  Plus,
  Receipt as ReceiptIcon,
  Search,
  Trash2,
  Upload,
} from "lucide-react";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { useProperties } from "@/lib/queries";
import { useAuth } from "@/lib/auth";
import {
  EXPENSE_CATEGORIES,
  expenses,
  isoDate,
  money,
  toCents,
  type Expense,
  type ExpenseCategory,
  type ExpenseInput,
} from "@/lib/backoffice";
import { errMsg, milesLabel, plural, useReady } from "@/lib/money-extra";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Stat, Tabs } from "@/components/ui/data-table";
import { fieldClass, Input, Label } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const CATEGORY_LABELS: Record<ExpenseCategory, string> = {
  materials: "Materials",
  mileage: "Mileage",
  fuel: "Fuel",
  equipment: "Equipment",
  repairs: "Repairs",
  vehicle: "Vehicle",
  insurance: "Insurance",
  payroll: "Payroll",
  marketing: "Marketing",
  software: "Software",
  licenses: "Licenses and permits",
  other: "Other",
};

const check = "size-4 accent-[var(--accent)]";
const full = cn(fieldClass, "h-11 w-full");

function monthRange(month: string): { from: string; to: string } {
  const [y, m] = month.split("-").map(Number);
  return { from: `${month}-01`, to: isoDate(new Date(y, m, 0)) };
}

function what(r: Expense) {
  return r.vendor || r.description || CATEGORY_LABELS[r.category];
}

export default function ExpensesPage() {
  const { can } = useAuth();
  const ready = useReady("expense:read");
  const allowed = useReady();
  const manage = can("expense:manage");
  const qc = useQueryClient();

  const [mode, setMode] = useState<"month" | "range">("month");
  const [month, setMonth] = useState(() => isoDate(new Date()).slice(0, 7));
  const [from, setFrom] = useState(() => monthRange(month).from);
  const [to, setTo] = useState(() => monthRange(month).to);
  const [category, setCategory] = useState("");
  const [toPayBack, setToPayBack] = useState(false);
  const [billable, setBillable] = useState(false);
  const [search, setSearch] = useState("");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const [dialog, setDialog] = useState<
    | { kind: "new" }
    | { kind: "trip" }
    | { kind: "edit"; expense: Expense }
    | null
  >(null);
  const [receiptsFor, setReceiptsFor] = useState<Expense | null>(null);

  const range = mode === "month" ? monthRange(month) : { from, to };
  const list = useQuery({
    queryKey: ["expenses", range.from, range.to, category],
    queryFn: () =>
      expenses.list({
        from: range.from || undefined,
        to: range.to || undefined,
        category: category || undefined,
      }),
    enabled: ready,
  });
  const reload = () => qc.invalidateQueries({ queryKey: ["expenses"] });

  const shown = useMemo(() => {
    const s = search.trim().toLowerCase();
    return (list.data ?? []).filter((r) => {
      if (toPayBack && !(r.reimbursable && !r.reimbursed)) return false;
      if (billable && !r.billable_to_owner) return false;
      if (s && !`${r.vendor ?? ""} ${r.description}`.toLowerCase().includes(s))
        return false;
      return true;
    });
  }, [list.data, toPayBack, billable, search]);

  const totals = useMemo(() => {
    const t = {
      total: 0,
      deductible: 0,
      miles: 0,
      mileage: 0,
      payBack: 0,
      payBackCount: 0,
      unbilled: 0,
      unbilledCount: 0,
    };
    for (const r of shown) {
      t.total += r.amount_cents;
      if (r.tax_deductible) t.deductible += r.amount_cents;
      if (r.category === "mileage") {
        t.miles += r.miles ?? 0;
        t.mileage += r.amount_cents;
      }
      if (r.reimbursable && !r.reimbursed) {
        t.payBack += r.amount_cents;
        t.payBackCount += 1;
      }
      if (r.billable_to_owner && !r.billed) {
        t.unbilled += r.amount_cents;
        t.unbilledCount += 1;
      }
    }
    return t;
  }, [shown]);

  const payable = (r: Expense) => r.reimbursable && !r.reimbursed;
  const selectable = shown.filter(payable);
  const selectedRows = selectable.filter((r) => selected.has(r.id));
  const allSelected =
    selectable.length > 0 && selectable.every((r) => selected.has(r.id));

  function toggle(id: string) {
    setSelected((s) => {
      const n = new Set(s);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });
  }

  async function markPaidBack() {
    if (selectedRows.length === 0) return;
    setBusy(true);
    try {
      const res = await expenses.reimburse(selectedRows.map((r) => r.id));
      toast.success(
        `Marked ${res.reimbursed} paid back · ${money(res.total_cents)}`
      );
      setSelected(new Set());
      void reload();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't mark them paid back"));
    } finally {
      setBusy(false);
    }
  }

  async function remove(r: Expense) {
    if (
      !window.confirm(
        `Delete this expense (${what(r)}, ${money(r.amount_cents)})?`
      )
    )
      return;
    try {
      await expenses.remove(r.id);
      toast.success("Expense deleted");
      void reload();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't delete it"));
    }
  }

  if (allowed && !ready)
    return (
      <div className="space-y-6">
        <PageHeader eyebrow="Money" title="Expenses" />
        <Panel>
          <EmptyState
            icon={<Lock />}
            title="You don't have access to expenses"
            description="Ask an admin for the expense:read permission."
          />
        </Panel>
      </div>
    );

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Money"
        title="Expenses"
        description="Receipts, materials and trips. Who needs paying back, and what to bill to owners."
        actions={
          manage && (
            <>
              <Button
                variant="secondary"
                onClick={() => setDialog({ kind: "trip" })}
              >
                <Car />
                Log a trip
              </Button>
              <Button onClick={() => setDialog({ kind: "new" })}>
                <Plus />
                New expense
              </Button>
            </>
          )
        }
      />

      <Panel className="flex flex-wrap items-end gap-3 p-4">
        <div className="space-y-1.5">
          <Label>Period</Label>
          <Tabs
            tabs={[
              ["month", "Month"],
              ["range", "Dates"],
            ]}
            value={mode}
            onChange={setMode}
          />
        </div>
        {mode === "month" ? (
          <div className="space-y-1.5">
            <Label htmlFor="exp-month">Month</Label>
            <input
              id="exp-month"
              type="month"
              className={cn(fieldClass, "block")}
              value={month}
              onChange={(e) => e.target.value && setMonth(e.target.value)}
            />
          </div>
        ) : (
          <>
            <div className="space-y-1.5">
              <Label htmlFor="exp-from">From</Label>
              <input
                id="exp-from"
                type="date"
                className={cn(fieldClass, "block")}
                value={from}
                onChange={(e) => setFrom(e.target.value)}
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="exp-to">To</Label>
              <input
                id="exp-to"
                type="date"
                className={cn(fieldClass, "block")}
                value={to}
                onChange={(e) => setTo(e.target.value)}
              />
            </div>
          </>
        )}
        <div className="space-y-1.5">
          <Label htmlFor="exp-cat">Category</Label>
          <select
            id="exp-cat"
            className={cn(fieldClass, "block")}
            value={category}
            onChange={(e) => setCategory(e.target.value)}
          >
            <option value="">All categories</option>
            {EXPENSE_CATEGORIES.map((c) => (
              <option key={c} value={c}>
                {CATEGORY_LABELS[c]}
              </option>
            ))}
          </select>
        </div>
        <div className="relative min-w-[200px] flex-1">
          <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-fg-3" />
          <Input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Vendor or description"
            aria-label="Search expenses"
            className="pl-9"
          />
        </div>
        <div className="flex flex-wrap gap-4 pb-2.5 text-[13px] text-fg-2">
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              className={check}
              checked={toPayBack}
              onChange={(e) => setToPayBack(e.target.checked)}
            />
            To pay back
          </label>
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              className={check}
              checked={billable}
              onChange={(e) => setBillable(e.target.checked)}
            />
            Billable to owners
          </label>
        </div>
      </Panel>

      <div className="grid grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-5">
        <Stat label="Total" value={money(totals.total)} />
        <Stat label="Tax deductible" value={money(totals.deductible)} />
        <Stat
          label="Mileage"
          value={money(totals.mileage)}
          hint={`${milesLabel(totals.miles)} miles`}
        />
        <Stat
          label="To pay back"
          value={money(totals.payBack)}
          hint={plural(totals.payBackCount, "expense")}
          tone={totals.payBack > 0 ? "warn" : undefined}
        />
        <Stat
          label="Billable, not billed"
          value={money(totals.unbilled)}
          hint={plural(totals.unbilledCount, "expense")}
        />
      </div>

      {list.error && (
        <Panel className="border-bad/30 p-4 text-[13px] text-bad">
          Couldn&apos;t load expenses: {list.error.message}
        </Panel>
      )}

      {manage && selectedRows.length > 0 && (
        <Panel className="flex flex-wrap items-center justify-between gap-3 border-accent/40 p-3">
          <span className="text-[13px] text-fg-2">
            {selectedRows.length} selected ·{" "}
            <span className="figure text-fg">
              {money(selectedRows.reduce((s, r) => s + r.amount_cents, 0))}
            </span>{" "}
            to pay back
          </span>
          <div className="flex gap-2">
            <Button
              size="sm"
              variant="ghost"
              onClick={() => setSelected(new Set())}
            >
              Clear
            </Button>
            <Button
              size="sm"
              loading={busy}
              onClick={() => void markPaidBack()}
            >
              <Check />
              Mark paid back
            </Button>
          </div>
        </Panel>
      )}

      <Panel className="overflow-hidden">
        {list.isLoading ? (
          <div className="space-y-2 p-4">
            {Array.from({ length: 6 }, (_, i) => (
              <Skeleton key={i} className="h-12" />
            ))}
          </div>
        ) : list.data && shown.length === 0 ? (
          <EmptyState
            icon={<ReceiptIcon />}
            title={
              list.data.length === 0
                ? "No expenses in this period"
                : "Nothing matches these filters"
            }
          />
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-[13px]">
              <thead>
                <tr className="border-b border-line text-left">
                  {manage && (
                    <th scope="col" className="w-10 px-4 py-2.5">
                      <input
                        type="checkbox"
                        className={check}
                        aria-label="Select everything waiting to be paid back"
                        title="Select everything waiting to be paid back"
                        checked={allSelected}
                        disabled={selectable.length === 0}
                        onChange={() =>
                          setSelected(
                            allSelected
                              ? new Set()
                              : new Set(selectable.map((r) => r.id))
                          )
                        }
                      />
                    </th>
                  )}
                  {["Date", "Category", "What", "Person", "For"].map((h) => (
                    <th
                      key={h}
                      scope="col"
                      className="eyebrow px-4 py-2.5 font-medium whitespace-nowrap"
                    >
                      {h}
                    </th>
                  ))}
                  <th
                    scope="col"
                    className="eyebrow px-4 py-2.5 text-right font-medium"
                  >
                    Amount
                  </th>
                  <th scope="col" className="eyebrow px-4 py-2.5 font-medium">
                    Status
                  </th>
                  <th scope="col" className="px-4 py-2.5">
                    <span className="sr-only">Actions</span>
                  </th>
                </tr>
              </thead>
              <tbody>
                {shown.map((r) => (
                  <tr
                    key={r.id}
                    className={cn(
                      "border-b border-line/60 align-top last:border-0 hover:bg-surface-2/60",
                      selected.has(r.id) && "bg-accent/5"
                    )}
                  >
                    {manage && (
                      <td className="px-4 py-2.5">
                        {payable(r) && (
                          <input
                            type="checkbox"
                            className={check}
                            aria-label="Select to mark paid back"
                            checked={selected.has(r.id)}
                            onChange={() => toggle(r.id)}
                          />
                        )}
                      </td>
                    )}
                    <td className="figure px-4 py-2.5 whitespace-nowrap text-fg-2">
                      {r.incurred_on}
                    </td>
                    <td className="px-4 py-2.5 whitespace-nowrap text-fg-2">
                      {CATEGORY_LABELS[r.category] ?? r.category}
                    </td>
                    <td className="min-w-[180px] px-4 py-2.5">
                      <div className="font-medium text-fg">
                        {r.vendor || r.description || "—"}
                      </div>
                      {r.vendor && r.description && (
                        <div className="text-xs text-fg-3">{r.description}</div>
                      )}
                      {r.category === "mileage" && r.miles != null && (
                        <div className="text-xs text-fg-3">
                          {r.miles} mi
                          {r.mileage_rate != null &&
                            ` at $${r.mileage_rate}/mi`}
                          {r.vehicle !== "none" && ` · ${r.vehicle} vehicle`}
                        </div>
                      )}
                    </td>
                    <td className="px-4 py-2.5 whitespace-nowrap text-fg-2">
                      {r.user_name ?? "Business"}
                    </td>
                    <td className="px-4 py-2.5 text-fg-2">
                      {r.maintenance_ticket_id ? (
                        <Link
                          href={`/console/maintenance/${r.maintenance_ticket_id}`}
                          className="hover:underline"
                        >
                          {r.work_order_title ?? "Work order"}
                        </Link>
                      ) : r.project_name ? (
                        <span>{r.project_name}</span>
                      ) : null}
                      {r.property_name && (
                        <div className="text-xs text-fg-3">
                          {r.property_name}
                        </div>
                      )}
                      {!r.maintenance_ticket_id &&
                        !r.project_name &&
                        !r.property_name && (
                          <span className="text-fg-4">—</span>
                        )}
                    </td>
                    <td className="figure px-4 py-2.5 text-right whitespace-nowrap text-fg">
                      {money(r.amount_cents)}
                    </td>
                    <td className="px-4 py-2.5">
                      <div className="flex flex-wrap gap-1">
                        {r.receipts > 0 ? (
                          <button
                            type="button"
                            onClick={() => setReceiptsFor(r)}
                          >
                            <Badge>
                              <Paperclip className="size-3" />
                              {r.receipts}
                            </Badge>
                          </button>
                        ) : r.tax_deductible && r.category !== "mileage" ? (
                          <Badge tone="warn">no receipt</Badge>
                        ) : null}
                        {r.reimbursable &&
                          (r.reimbursed ? (
                            <Badge tone="good">paid back</Badge>
                          ) : (
                            <Badge tone="info">to pay back</Badge>
                          ))}
                        {r.billable_to_owner &&
                          (r.billed ? (
                            <Badge tone="good">billed</Badge>
                          ) : (
                            <Badge tone="accent">billable</Badge>
                          ))}
                        {!r.tax_deductible && <Badge>not deductible</Badge>}
                      </div>
                    </td>
                    <td className="px-4 py-2 text-right whitespace-nowrap">
                      <div className="flex justify-end gap-0.5">
                        <Button
                          size="icon"
                          variant="ghost"
                          aria-label="Receipts"
                          title="Receipts"
                          onClick={() => setReceiptsFor(r)}
                        >
                          <FileText />
                        </Button>
                        {manage && (
                          <>
                            <Button
                              size="icon"
                              variant="ghost"
                              aria-label="Edit"
                              title="Edit"
                              onClick={() =>
                                setDialog({ kind: "edit", expense: r })
                              }
                            >
                              <Pencil />
                            </Button>
                            <Button
                              size="icon"
                              variant="ghost"
                              aria-label="Delete"
                              title="Delete"
                              className="hover:text-bad"
                              onClick={() => void remove(r)}
                            >
                              <Trash2 />
                            </Button>
                          </>
                        )}
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Panel>

      {dialog && (
        <ExpenseDialog
          key={dialog.kind === "edit" ? dialog.expense.id : dialog.kind}
          initial={dialog.kind === "edit" ? dialog.expense : undefined}
          trip={dialog.kind === "trip"}
          onClose={() => setDialog(null)}
          onSaved={() => {
            setDialog(null);
            void reload();
          }}
        />
      )}

      {receiptsFor && (
        <ReceiptsDialog
          key={receiptsFor.id}
          expense={receiptsFor}
          manage={manage}
          onClose={() => setReceiptsFor(null)}
          onChanged={() => void reload()}
        />
      )}
    </div>
  );
}

// ---- new / edit / trip ------------------------------------------------------

function str(v: unknown): string {
  return typeof v === "string" || typeof v === "number" ? String(v) : "";
}

function ExpenseDialog({
  initial,
  trip,
  onClose,
  onSaved,
}: {
  initial?: Expense;
  trip?: boolean;
  onClose: () => void;
  onSaved: () => void;
}) {
  const scoped = useReady();
  const d = initial?.details ?? {};
  const [incurredOn, setIncurredOn] = useState(
    initial?.incurred_on ?? isoDate(new Date())
  );
  const [category, setCategory] = useState<ExpenseCategory>(
    initial?.category ?? (trip ? "mileage" : "materials")
  );
  const [vendor, setVendor] = useState(initial?.vendor ?? "");
  const [description, setDescription] = useState(initial?.description ?? "");
  const [amount, setAmount] = useState(
    initial && initial.category !== "mileage"
      ? (initial.amount_cents / 100).toFixed(2)
      : ""
  );
  const [taxDeductible, setTaxDeductible] = useState(
    initial?.tax_deductible ?? true
  );
  const [vehicle, setVehicle] = useState<Expense["vehicle"]>(
    initial?.vehicle ?? (trip ? "personal" : "none")
  );
  const [reimbursable, setReimbursable] = useState(
    initial?.reimbursable ?? false
  );
  const [billable, setBillable] = useState(initial?.billable_to_owner ?? false);
  const [propertyId, setPropertyId] = useState(initial?.property_id ?? "");
  const [ticketId, setTicketId] = useState(
    initial?.maintenance_ticket_id ?? ""
  );
  const [projectId, setProjectId] = useState(initial?.rehab_project_id ?? "");

  // Mileage: miles typed (doubled for a round trip) or odometer readings.
  const hasOdo = d.odometer_start != null && d.odometer_end != null;
  const [milesMode, setMilesMode] = useState<"miles" | "odometer">(
    hasOdo ? "odometer" : "miles"
  );
  const [roundTrip, setRoundTrip] = useState(d.round_trip === true);
  const [oneWay, setOneWay] = useState(() => {
    if (initial?.miles == null || hasOdo) return "";
    return String(d.round_trip === true ? initial.miles / 2 : initial.miles);
  });
  const [odoStart, setOdoStart] = useState(str(d.odometer_start));
  const [odoEnd, setOdoEnd] = useState(str(d.odometer_end));
  const [tripFrom, setTripFrom] = useState(str(d.from));
  const [tripTo, setTripTo] = useState(str(d.to));
  const [busy, setBusy] = useState(false);

  const properties = useProperties({ enabled: scoped });
  const tickets = useQuery({
    queryKey: ["tickets"],
    queryFn: () => api.tickets(),
    enabled: scoped,
  });
  const projects = useQuery({
    queryKey: ["rehab-projects", propertyId],
    queryFn: () => api.rehabProjects(propertyId),
    enabled: scoped && !!propertyId,
  });
  const projectList = propertyId ? (projects.data ?? []) : [];

  const isMileage = category === "mileage";
  const miles = useMemo(() => {
    if (milesMode === "odometer") {
      const a = Number(odoStart);
      const b = Number(odoEnd);
      return odoStart && odoEnd && b >= a ? b - a : null;
    }
    const n = Number(oneWay);
    return oneWay.trim() && Number.isFinite(n) && n > 0
      ? n * (roundTrip ? 2 : 1)
      : null;
  }, [milesMode, odoStart, odoEnd, oneWay, roundTrip]);

  const allTickets = tickets.data ?? [];
  const ticketChoices = propertyId
    ? allTickets.filter((t) => t.property_id === propertyId)
    : allTickets;

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    const details: Record<string, unknown> = { ...d };
    let amountCents: number | null | undefined;
    if (isMileage) {
      if (miles == null) {
        toast.error(
          milesMode === "odometer"
            ? "Enter both odometer readings. The end has to be higher."
            : "How many miles was the trip?"
        );
        return;
      }
      delete details.odometer_start;
      delete details.odometer_end;
      if (milesMode === "odometer") {
        details.odometer_start = Number(odoStart);
        details.odometer_end = Number(odoEnd);
        details.round_trip = false;
      } else {
        details.round_trip = roundTrip;
      }
      if (tripFrom.trim()) details.from = tripFrom.trim();
      else delete details.from;
      if (tripTo.trim()) details.to = tripTo.trim();
      else delete details.to;
      amountCents = undefined; // the server prices mileage at the IRS rate
    } else {
      amountCents = toCents(amount);
      if (amountCents == null || amountCents < 0) {
        toast.error("Enter the amount, like 42.50.");
        return;
      }
    }
    if (!isMileage && !vendor.trim() && !description.trim()) {
      toast.error("Add a vendor or a short description.");
      return;
    }
    const body: ExpenseInput = {
      incurred_on: incurredOn,
      category,
      vendor: vendor.trim() || null,
      description: description.trim(),
      amount_cents: amountCents,
      miles: isMileage ? miles : null,
      tax_deductible: taxDeductible,
      vehicle: isMileage && vehicle === "none" ? "personal" : vehicle,
      reimbursable,
      billable_to_owner: billable,
      property_id: propertyId || null,
      maintenance_ticket_id: ticketId || null,
      rehab_project_id: projectId || null,
      details,
    };
    setBusy(true);
    try {
      if (initial) {
        await expenses.update(initial.id, body);
        toast.success("Expense saved");
      } else {
        const created = await expenses.create(body);
        toast.success(
          isMileage
            ? `Trip logged · ${created.miles ?? miles} mi · ${money(created.amount_cents)}`
            : "Expense added"
        );
      }
      onSaved();
    } catch (err) {
      toast.error(errMsg(err, "Couldn't save the expense"));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-h-[90vh] max-w-2xl overflow-y-auto">
        <form onSubmit={submit} className="space-y-4">
          <div>
            <DialogTitle className="text-[17px] font-semibold">
              {initial ? "Edit expense" : trip ? "Log a trip" : "New expense"}
            </DialogTitle>
            <DialogDescription className="mt-1 text-[13px] text-fg-3">
              {isMileage
                ? "Enter the miles or the odometer readings. The amount is worked out at the mileage rate."
                : "What was bought, where, and who it's for."}
            </DialogDescription>
          </div>

          <div className="grid gap-3 sm:grid-cols-2">
            <Labeled label="Date">
              <input
                type="date"
                className={full}
                value={incurredOn}
                onChange={(e) => setIncurredOn(e.target.value)}
                required
              />
            </Labeled>
            <Labeled label="Category">
              <select
                className={full}
                value={category}
                onChange={(e) => {
                  const c = e.target.value as ExpenseCategory;
                  setCategory(c);
                  if (c === "mileage" && vehicle === "none")
                    setVehicle("personal");
                }}
              >
                {EXPENSE_CATEGORIES.map((c) => (
                  <option key={c} value={c}>
                    {CATEGORY_LABELS[c]}
                  </option>
                ))}
              </select>
            </Labeled>
            {!isMileage && (
              <Labeled label="Vendor">
                <input
                  className={full}
                  placeholder="Home Depot"
                  value={vendor}
                  onChange={(e) => setVendor(e.target.value)}
                />
              </Labeled>
            )}
            <Labeled
              label={isMileage ? "Why the trip" : "Description"}
              className={isMileage ? "sm:col-span-2" : undefined}
            >
              <input
                className={full}
                placeholder={
                  isMileage
                    ? "Pick up water heater, then 12 Oak St"
                    : "Faucet and supply lines"
                }
                value={description}
                onChange={(e) => setDescription(e.target.value)}
              />
            </Labeled>
          </div>

          {isMileage ? (
            <div className="space-y-3 rounded-xl border border-line p-3">
              <Tabs
                tabs={[
                  ["miles", "Miles"],
                  ["odometer", "Odometer"],
                ]}
                value={milesMode}
                onChange={setMilesMode}
                className="w-fit"
              />
              <div className="grid gap-3 sm:grid-cols-2">
                <Labeled label="From">
                  <input
                    className={full}
                    placeholder="Office"
                    value={tripFrom}
                    onChange={(e) => setTripFrom(e.target.value)}
                  />
                </Labeled>
                <Labeled label="To">
                  <input
                    className={full}
                    placeholder="12 Oak St"
                    value={tripTo}
                    onChange={(e) => setTripTo(e.target.value)}
                  />
                </Labeled>
                {milesMode === "miles" ? (
                  <>
                    <Labeled label={roundTrip ? "Miles (one way)" : "Miles"}>
                      <input
                        className={full}
                        inputMode="decimal"
                        placeholder="0.0"
                        value={oneWay}
                        onChange={(e) => setOneWay(e.target.value)}
                      />
                    </Labeled>
                    <label className="flex items-center gap-2 self-end pb-3 text-[13px] text-fg-2">
                      <input
                        type="checkbox"
                        className={check}
                        checked={roundTrip}
                        onChange={(e) => setRoundTrip(e.target.checked)}
                      />
                      Round trip (doubles the miles)
                    </label>
                  </>
                ) : (
                  <>
                    <Labeled label="Odometer start">
                      <input
                        className={full}
                        inputMode="decimal"
                        value={odoStart}
                        onChange={(e) => setOdoStart(e.target.value)}
                      />
                    </Labeled>
                    <Labeled label="Odometer end">
                      <input
                        className={full}
                        inputMode="decimal"
                        value={odoEnd}
                        onChange={(e) => setOdoEnd(e.target.value)}
                      />
                    </Labeled>
                  </>
                )}
                <Labeled label="Vehicle">
                  <select
                    className={full}
                    value={vehicle}
                    onChange={(e) =>
                      setVehicle(e.target.value as Expense["vehicle"])
                    }
                  >
                    <option value="personal">Personal vehicle</option>
                    <option value="company">Company vehicle</option>
                  </select>
                </Labeled>
              </div>
              <p className="text-[13px] text-fg-2">
                {miles != null ? (
                  <>
                    <strong className="text-fg">
                      {milesLabel(miles)} miles
                    </strong>
                    . The amount is worked out when you save.
                  </>
                ) : (
                  <span className="text-fg-3">
                    Enter the miles to see the total.
                  </span>
                )}
              </p>
            </div>
          ) : (
            <div className="grid gap-3 sm:grid-cols-2">
              <Labeled label="Amount">
                <input
                  className={full}
                  inputMode="decimal"
                  placeholder="0.00"
                  value={amount}
                  onChange={(e) => setAmount(e.target.value)}
                />
              </Labeled>
              <Labeled label="Vehicle">
                <select
                  className={full}
                  value={vehicle}
                  onChange={(e) =>
                    setVehicle(e.target.value as Expense["vehicle"])
                  }
                >
                  <option value="none">Not a vehicle cost</option>
                  <option value="company">Company vehicle</option>
                  <option value="personal">Personal vehicle</option>
                </select>
              </Labeled>
            </div>
          )}

          <div className="grid gap-3 sm:grid-cols-3">
            <Labeled label="Property">
              <select
                className={full}
                value={propertyId}
                onChange={(e) => {
                  setPropertyId(e.target.value);
                  setTicketId("");
                  setProjectId("");
                }}
              >
                <option value="">None</option>
                {properties.data?.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </select>
            </Labeled>
            <Labeled label="Work order">
              <select
                className={full}
                value={ticketId}
                onChange={(e) => {
                  setTicketId(e.target.value);
                  const t = allTickets.find((x) => x.id === e.target.value);
                  if (t && !propertyId) setPropertyId(t.property_id);
                }}
              >
                <option value="">None</option>
                {ticketId &&
                  !ticketChoices.some((t) => t.id === ticketId) &&
                  initial?.work_order_title && (
                    <option value={ticketId}>{initial.work_order_title}</option>
                  )}
                {ticketChoices.map((t) => (
                  <option key={t.id} value={t.id}>
                    {t.title}
                  </option>
                ))}
              </select>
            </Labeled>
            <Labeled label="Rehab project">
              <select
                className={full}
                value={projectId}
                disabled={!propertyId && !projectId}
                onChange={(e) => setProjectId(e.target.value)}
              >
                <option value="">
                  {propertyId ? "None" : "Pick a property first"}
                </option>
                {projectId &&
                  !projectList.some((p) => p.id === projectId) &&
                  initial?.project_name && (
                    <option value={projectId}>{initial.project_name}</option>
                  )}
                {projectList.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </select>
            </Labeled>
          </div>

          <div className="flex flex-wrap gap-x-5 gap-y-2 text-[13px] text-fg-2">
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                className={check}
                checked={taxDeductible}
                onChange={(e) => setTaxDeductible(e.target.checked)}
              />
              Tax deductible
            </label>
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                className={check}
                checked={reimbursable}
                onChange={(e) => setReimbursable(e.target.checked)}
              />
              Someone paid out of pocket (pay them back)
            </label>
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                className={check}
                checked={billable}
                onChange={(e) => setBillable(e.target.checked)}
              />
              Bill it to the owner
            </label>
          </div>

          <div className="flex justify-end gap-2 pt-2">
            <Button type="button" variant="ghost" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" loading={busy}>
              {initial ? "Save" : isMileage ? "Log trip" : "Add expense"}
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function Labeled({
  label,
  className,
  children,
}: {
  label: string;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <label className={cn("block space-y-1.5", className)}>
      <span className="text-[13px] font-medium text-fg-2">{label}</span>
      {children}
    </label>
  );
}

// ---- receipts ---------------------------------------------------------------

function ReceiptsDialog({
  expense,
  manage,
  onClose,
  onChanged,
}: {
  expense: Expense;
  manage: boolean;
  onClose: () => void;
  onChanged: () => void;
}) {
  const qc = useQueryClient();
  const key = ["expense-receipts", expense.id];
  const list = useQuery({
    queryKey: key,
    queryFn: () => expenses.receipts(expense.id),
  });
  const [busy, setBusy] = useState(false);

  async function upload(file: File) {
    setBusy(true);
    try {
      await expenses.uploadReceipt(expense.id, file);
      toast.success("Receipt added");
      void qc.invalidateQueries({ queryKey: key });
      onChanged();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't upload the receipt"));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md">
        <DialogTitle className="text-[17px] font-semibold">
          Receipts
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          {expense.vendor || expense.description || "Expense"} ·{" "}
          {expense.incurred_on} · {money(expense.amount_cents)}
        </DialogDescription>
        <div className="mt-4 space-y-2 text-[13px]">
          {list.isLoading && <Skeleton className="h-10" />}
          {list.error && <p className="text-bad">{list.error.message}</p>}
          {list.data?.length === 0 && (
            <p className="text-fg-3">No receipts on this expense yet.</p>
          )}
          {list.data?.map((r) => (
            <a
              key={r.document_id}
              href={r.download_url}
              target="_blank"
              rel="noopener noreferrer"
              className="flex items-center justify-between gap-3 rounded-xl border border-line px-3 py-2 transition hover:border-accent hover:bg-fill-2"
            >
              <span className="flex min-w-0 items-center gap-2 font-medium text-fg">
                <FileText className="size-4 shrink-0 text-fg-3" />
                <span className="truncate">{r.filename}</span>
              </span>
              <span className="figure shrink-0 text-xs text-fg-3">
                {r.created_at.slice(0, 10)}
              </span>
            </a>
          ))}
        </div>
        <div className="mt-5 flex justify-end gap-2">
          <Button variant="ghost" onClick={onClose}>
            Close
          </Button>
          {manage && (
            <Button asChild disabled={busy}>
              <label
                className={cn(
                  "cursor-pointer",
                  busy && "pointer-events-none opacity-45"
                )}
              >
                <Upload />
                {busy ? "Uploading…" : "Add receipt"}
                <input
                  type="file"
                  accept="image/*,application/pdf"
                  className="sr-only"
                  onChange={(e) => {
                    const f = e.target.files?.[0];
                    e.target.value = "";
                    if (f) void upload(f);
                  }}
                />
              </label>
            </Button>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
