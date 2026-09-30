"use client";

// Expenses & mileage: everything the business spends — receipts, trips,
// materials — with who needs paying back and what can be billed to owners.
// Gated by `expense:read`; adding, editing and paying back need
// `expense:manage`.

import { useCallback, useEffect, useMemo, useState } from "react";
import Link from "next/link";
import { toast } from "sonner";
import { api } from "@/lib/api";
import type { MaintenanceTicket, Property } from "@/lib/types";
import {
  EXPENSE_CATEGORIES,
  expenses,
  isoDate,
  money,
  toCents,
  type Expense,
  type ExpenseCategory,
  type ExpenseInput,
  type Receipt,
} from "@/lib/backoffice";
import { useAuth } from "@/lib/auth";
import { logError } from "@/lib/log";
import { Badge, Button, Card } from "@/components/ui";
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
const select =
  "rounded-xl border border-line bg-surface px-3 py-2 text-sm text-ink";
const label = "flex flex-col gap-1 text-xs font-semibold text-ink-3";

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
  licenses: "Licenses & permits",
  other: "Other",
};

function monthRange(month: string): { from: string; to: string } {
  const [y, m] = month.split("-").map(Number);
  return {
    from: `${month}-01`,
    to: isoDate(new Date(y, m, 0)),
  };
}

function errMsg(e: unknown, fallback = "Something went wrong") {
  return e instanceof Error ? e.message : fallback;
}

export default function ExpensesPage() {
  const { can } = useAuth();
  const read = can("expense:read");
  const manage = can("expense:manage");

  const [mode, setMode] = useState<"month" | "range">("month");
  const [month, setMonth] = useState(() => isoDate(new Date()).slice(0, 7));
  const [from, setFrom] = useState(() => monthRange(month).from);
  const [to, setTo] = useState(() => monthRange(month).to);
  const [category, setCategory] = useState("");
  const [toPayBack, setToPayBack] = useState(false);
  const [billable, setBillable] = useState(false);
  const [search, setSearch] = useState("");

  const [rows, setRows] = useState<Expense[] | null>(null);
  const [error, setError] = useState<string | null>(null);
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

  const load = useCallback(() => {
    expenses
      .list({
        from: range.from || undefined,
        to: range.to || undefined,
        category: category || undefined,
      })
      .then((r) => {
        setRows(r);
        setError(null);
      })
      .catch((e) => setError(errMsg(e, "Couldn't load expenses")));
  }, [range.from, range.to, category]);

  useEffect(() => {
    if (!read) return;
    load();
  }, [load, read]);

  const shown = useMemo(() => {
    const s = search.trim().toLowerCase();
    return (rows ?? []).filter((r) => {
      if (toPayBack && !(r.reimbursable && !r.reimbursed)) return false;
      if (billable && !r.billable_to_owner) return false;
      if (s && !`${r.vendor ?? ""} ${r.description}`.toLowerCase().includes(s))
        return false;
      return true;
    });
  }, [rows, toPayBack, billable, search]);

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
  const selectedRows = shown.filter((r) => selected.has(r.id) && payable(r));
  const selectable = shown.filter(payable);
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
      load();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't mark them paid back"));
    } finally {
      setBusy(false);
    }
  }

  async function remove(r: Expense) {
    if (
      !window.confirm(
        `Delete this expense (${r.vendor || r.description || CATEGORY_LABELS[r.category]}, ${money(r.amount_cents)})?`
      )
    )
      return;
    try {
      await expenses.remove(r.id);
      toast.success("Expense deleted");
      load();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't delete it"));
    }
  }

  if (!read) {
    return (
      <Card className="p-6">
        <p className="text-ink-2">
          You don&apos;t have access to expenses. Ask an admin for the{" "}
          <span className="font-mono">expense:read</span> permission.
        </p>
      </Card>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            Expenses
          </h1>
          <p className="text-ink-3">
            Receipts, materials and trips — who needs paying back, and what to
            bill to owners.
          </p>
        </div>
        {manage && (
          <div className="flex gap-2">
            <Button
              variant="outline"
              onClick={() => setDialog({ kind: "trip" })}
            >
              Log a trip
            </Button>
            <Button onClick={() => setDialog({ kind: "new" })}>
              New expense
            </Button>
          </div>
        )}
      </div>

      {/* Filters */}
      <Card className="flex flex-wrap items-end gap-3 p-4">
        <label className={label}>
          Period
          <select
            className={select}
            value={mode}
            onChange={(e) => setMode(e.target.value as "month" | "range")}
          >
            <option value="month">Month</option>
            <option value="range">Date range</option>
          </select>
        </label>
        {mode === "month" ? (
          <label className={label}>
            Month
            <input
              type="month"
              className={select}
              value={month}
              onChange={(e) => e.target.value && setMonth(e.target.value)}
            />
          </label>
        ) : (
          <>
            <label className={label}>
              From
              <input
                type="date"
                className={select}
                value={from}
                onChange={(e) => setFrom(e.target.value)}
              />
            </label>
            <label className={label}>
              To
              <input
                type="date"
                className={select}
                value={to}
                onChange={(e) => setTo(e.target.value)}
              />
            </label>
          </>
        )}
        <label className={label}>
          Category
          <select
            className={select}
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
        </label>
        <label className={`${label} min-w-[200px] flex-1`}>
          Search
          <input
            className={field}
            placeholder="Vendor or description"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </label>
        <div className="flex flex-wrap gap-3 pb-2 text-sm text-ink-2">
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={toPayBack}
              onChange={(e) => setToPayBack(e.target.checked)}
            />
            To pay back
          </label>
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={billable}
              onChange={(e) => setBillable(e.target.checked)}
            />
            Billable to owners
          </label>
        </div>
      </Card>

      {/* Totals */}
      <div className="grid grid-cols-2 gap-3 md:grid-cols-5">
        <Tile label="Total" value={money(totals.total)} />
        <Tile label="Tax deductible" value={money(totals.deductible)} />
        <Tile
          label="Mileage"
          value={money(totals.mileage)}
          sub={`${totals.miles.toLocaleString("en-US", { maximumFractionDigits: 1 })} miles`}
        />
        <Tile
          label="Waiting to be paid back"
          value={money(totals.payBack)}
          sub={`${totals.payBackCount} expense${totals.payBackCount === 1 ? "" : "s"}`}
          tone={totals.payBack > 0 ? "warn" : undefined}
        />
        <Tile
          label="Billable, not billed yet"
          value={money(totals.unbilled)}
          sub={`${totals.unbilledCount} expense${totals.unbilledCount === 1 ? "" : "s"}`}
        />
      </div>

      {error && <p className="text-bad">{error}</p>}

      {manage && selectedRows.length > 0 && (
        <Card className="flex flex-wrap items-center justify-between gap-3 p-3">
          <span className="text-sm text-ink-2">
            {selectedRows.length} selected ·{" "}
            {money(selectedRows.reduce((s, r) => s + r.amount_cents, 0))} to pay
            back
          </span>
          <div className="flex gap-2">
            <Button variant="ghost" onClick={() => setSelected(new Set())}>
              Clear
            </Button>
            <Button disabled={busy} onClick={() => void markPaidBack()}>
              {busy ? "Saving…" : "Mark paid back"}
            </Button>
          </div>
        </Card>
      )}

      <Card className="overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-line text-left text-xs uppercase tracking-wide text-ink-3">
                {manage && (
                  <th className="px-3 py-2">
                    <input
                      type="checkbox"
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
                <th className="px-3 py-2 font-semibold">Date</th>
                <th className="px-3 py-2 font-semibold">Category</th>
                <th className="px-3 py-2 font-semibold">Vendor / what</th>
                <th className="px-3 py-2 font-semibold">Person</th>
                <th className="px-3 py-2 font-semibold">For</th>
                <th className="px-3 py-2 text-right font-semibold">Amount</th>
                <th className="px-3 py-2 font-semibold">Status</th>
                <th className="px-3 py-2" />
              </tr>
            </thead>
            <tbody className="divide-y divide-line">
              {shown.map((r) => (
                <tr key={r.id} className="align-top">
                  {manage && (
                    <td className="px-3 py-2.5">
                      {payable(r) && (
                        <input
                          type="checkbox"
                          aria-label="Select to mark paid back"
                          checked={selected.has(r.id)}
                          onChange={() => toggle(r.id)}
                        />
                      )}
                    </td>
                  )}
                  <td className="whitespace-nowrap px-3 py-2.5 tabular-nums">
                    {r.incurred_on}
                  </td>
                  <td className="whitespace-nowrap px-3 py-2.5">
                    {CATEGORY_LABELS[r.category] ?? r.category}
                  </td>
                  <td className="min-w-[180px] px-3 py-2.5">
                    <div className="font-semibold">
                      {r.vendor || r.description || "—"}
                    </div>
                    {r.vendor && r.description && (
                      <div className="text-xs text-ink-3">{r.description}</div>
                    )}
                    {r.category === "mileage" && r.miles != null && (
                      <div className="text-xs text-ink-3">
                        {r.miles} mi
                        {r.mileage_rate != null && ` × $${r.mileage_rate}/mi`}
                        {r.vehicle !== "none" && ` · ${r.vehicle} vehicle`}
                      </div>
                    )}
                  </td>
                  <td className="whitespace-nowrap px-3 py-2.5 text-ink-2">
                    {r.user_name ?? "Business"}
                  </td>
                  <td className="px-3 py-2.5 text-ink-2">
                    {r.maintenance_ticket_id ? (
                      <Link
                        href={`/console/maintenance/${r.maintenance_ticket_id}`}
                        className="underline"
                      >
                        {r.work_order_title ?? "Work order"}
                      </Link>
                    ) : r.project_name ? (
                      <span>{r.project_name}</span>
                    ) : null}
                    {r.property_name && (
                      <div className="text-xs text-ink-3">
                        {r.property_name}
                      </div>
                    )}
                    {!r.maintenance_ticket_id &&
                      !r.project_name &&
                      !r.property_name && <span className="text-ink-3">—</span>}
                  </td>
                  <td className="whitespace-nowrap px-3 py-2.5 text-right font-mono">
                    {money(r.amount_cents)}
                  </td>
                  <td className="px-3 py-2.5">
                    <div className="flex flex-wrap gap-1">
                      {r.receipts > 0 ? (
                        <button onClick={() => setReceiptsFor(r)}>
                          <Badge tone="neutral">
                            {r.receipts} receipt{r.receipts === 1 ? "" : "s"}
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
                      {!r.tax_deductible && (
                        <Badge tone="neutral">not deductible</Badge>
                      )}
                    </div>
                  </td>
                  <td className="whitespace-nowrap px-3 py-2.5 text-right">
                    <div className="flex justify-end gap-1">
                      <button
                        onClick={() => setReceiptsFor(r)}
                        className="rounded-lg border border-line px-2 py-1 text-xs font-semibold text-ink-2 hover:border-accent"
                      >
                        Receipts
                      </button>
                      {manage && (
                        <>
                          <button
                            onClick={() =>
                              setDialog({ kind: "edit", expense: r })
                            }
                            className="rounded-lg border border-line px-2 py-1 text-xs font-semibold text-ink-2 hover:border-accent"
                          >
                            Edit
                          </button>
                          <button
                            onClick={() => void remove(r)}
                            className="rounded-lg px-2 py-1 text-xs font-semibold text-bad hover:bg-bad-soft"
                          >
                            Delete
                          </button>
                        </>
                      )}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {rows === null && !error && (
          <div className="px-5 py-10 text-center text-ink-3">Loading…</div>
        )}
        {rows !== null && shown.length === 0 && (
          <div className="px-5 py-10 text-center text-ink-3">
            {rows.length === 0
              ? "No expenses in this period yet."
              : "Nothing matches these filters."}
          </div>
        )}
      </Card>

      {dialog && (
        <ExpenseDialog
          key={dialog.kind === "edit" ? dialog.expense.id : dialog.kind}
          initial={dialog.kind === "edit" ? dialog.expense : undefined}
          trip={dialog.kind === "trip"}
          onClose={() => setDialog(null)}
          onSaved={() => {
            setDialog(null);
            load();
          }}
        />
      )}

      {receiptsFor && (
        <ReceiptsDialog
          key={receiptsFor.id}
          expense={receiptsFor}
          manage={manage}
          onClose={() => setReceiptsFor(null)}
          onChanged={load}
        />
      )}
    </div>
  );
}

function Tile({
  label,
  value,
  sub,
  tone,
}: {
  label: string;
  value: string;
  sub?: string;
  tone?: "warn";
}) {
  return (
    <Card className="p-4">
      <div className="mb-2 text-xs font-semibold uppercase tracking-wide text-ink-3">
        {label}
      </div>
      <div
        className={`font-display text-2xl font-extrabold tracking-tight ${tone === "warn" ? "text-warn" : ""}`}
      >
        {value}
      </div>
      {sub && <div className="mt-0.5 text-xs text-ink-3">{sub}</div>}
    </Card>
  );
}

// ---------------------------------------------------------------------------
// New / edit / trip dialog
// ---------------------------------------------------------------------------

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

  // Mileage: typed miles (optionally doubled for a round trip) or odometer.
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

  const [properties, setProperties] = useState<Property[]>([]);
  const [tickets, setTickets] = useState<MaintenanceTicket[]>([]);
  const [projects, setProjects] = useState<{ id: string; name: string }[]>([]);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api
      .properties()
      .then(setProperties)
      .catch((e) => logError("failed to load properties", e));
    api
      .tickets()
      .then(setTickets)
      .catch((e) => logError("failed to load work orders", e));
  }, []);

  useEffect(() => {
    if (!propertyId) return;
    api
      .rehabProjects(propertyId)
      .then((ps) => setProjects(ps.map((p) => ({ id: p.id, name: p.name }))))
      .catch((e) => logError("failed to load projects", e));
  }, [propertyId]);

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

  const ticketChoices = propertyId
    ? tickets.filter((t) => t.property_id === propertyId)
    : tickets;

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    const details: Record<string, unknown> = { ...d };
    let amountCents: number | null | undefined;
    if (isMileage) {
      if (miles == null) {
        toast.error(
          milesMode === "odometer"
            ? "Enter both odometer readings (the end has to be higher)."
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
          <DialogHeader>
            <DialogTitle>
              {initial ? "Edit expense" : trip ? "Log a trip" : "New expense"}
            </DialogTitle>
            <DialogDescription>
              {isMileage
                ? "Enter the miles (or odometer readings) — we work out the amount at the mileage rate."
                : "What was bought, where, and who it's for."}
            </DialogDescription>
          </DialogHeader>

          <div className="grid gap-3 sm:grid-cols-2">
            <label className={label}>
              Date
              <input
                type="date"
                className={field}
                value={incurredOn}
                onChange={(e) => setIncurredOn(e.target.value)}
                required
              />
            </label>
            <label className={label}>
              Category
              <select
                className={field}
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
            </label>
            {!isMileage && (
              <label className={label}>
                Vendor
                <input
                  className={field}
                  placeholder="e.g. Home Depot"
                  value={vendor}
                  onChange={(e) => setVendor(e.target.value)}
                />
              </label>
            )}
            <label className={`${label} ${isMileage ? "sm:col-span-2" : ""}`}>
              {isMileage ? "Why the trip" : "Description"}
              <input
                className={field}
                placeholder={
                  isMileage
                    ? "e.g. Pick up water heater, then 12 Oak St"
                    : "e.g. Faucet and supply lines"
                }
                value={description}
                onChange={(e) => setDescription(e.target.value)}
              />
            </label>
          </div>

          {isMileage ? (
            <div className="space-y-3 rounded-xl border border-line p-3">
              <div className="flex gap-2 text-sm">
                {(["miles", "odometer"] as const).map((m) => (
                  <button
                    key={m}
                    type="button"
                    onClick={() => setMilesMode(m)}
                    className={`rounded-lg px-3 py-1.5 font-semibold ${
                      milesMode === m
                        ? "bg-accent-soft text-accent-2"
                        : "text-ink-3 hover:bg-surface-2"
                    }`}
                  >
                    {m === "miles" ? "Miles" : "Odometer"}
                  </button>
                ))}
              </div>
              <div className="grid gap-3 sm:grid-cols-2">
                <label className={label}>
                  From
                  <input
                    className={field}
                    placeholder="Office"
                    value={tripFrom}
                    onChange={(e) => setTripFrom(e.target.value)}
                  />
                </label>
                <label className={label}>
                  To
                  <input
                    className={field}
                    placeholder="12 Oak St"
                    value={tripTo}
                    onChange={(e) => setTripTo(e.target.value)}
                  />
                </label>
                {milesMode === "miles" ? (
                  <>
                    <label className={label}>
                      Miles {roundTrip ? "(one way)" : ""}
                      <input
                        className={field}
                        inputMode="decimal"
                        placeholder="0.0"
                        value={oneWay}
                        onChange={(e) => setOneWay(e.target.value)}
                      />
                    </label>
                    <label className="flex items-center gap-2 self-end pb-2 text-sm text-ink-2">
                      <input
                        type="checkbox"
                        checked={roundTrip}
                        onChange={(e) => setRoundTrip(e.target.checked)}
                      />
                      Round trip (doubles the miles)
                    </label>
                  </>
                ) : (
                  <>
                    <label className={label}>
                      Odometer start
                      <input
                        className={field}
                        inputMode="decimal"
                        value={odoStart}
                        onChange={(e) => setOdoStart(e.target.value)}
                      />
                    </label>
                    <label className={label}>
                      Odometer end
                      <input
                        className={field}
                        inputMode="decimal"
                        value={odoEnd}
                        onChange={(e) => setOdoEnd(e.target.value)}
                      />
                    </label>
                  </>
                )}
                <label className={label}>
                  Vehicle
                  <select
                    className={field}
                    value={vehicle}
                    onChange={(e) =>
                      setVehicle(e.target.value as Expense["vehicle"])
                    }
                  >
                    <option value="personal">Personal vehicle</option>
                    <option value="company">Company vehicle</option>
                  </select>
                </label>
              </div>
              <p className="text-sm text-ink-2">
                {miles != null ? (
                  <>
                    <strong>
                      {miles.toLocaleString("en-US", {
                        maximumFractionDigits: 1,
                      })}{" "}
                      miles
                    </strong>{" "}
                    — the amount is worked out when you save.
                  </>
                ) : (
                  <span className="text-ink-3">
                    Enter the miles to see the total.
                  </span>
                )}
              </p>
            </div>
          ) : (
            <div className="grid gap-3 sm:grid-cols-2">
              <label className={label}>
                Amount
                <input
                  className={field}
                  inputMode="decimal"
                  placeholder="0.00"
                  value={amount}
                  onChange={(e) => setAmount(e.target.value)}
                />
              </label>
              <label className={label}>
                Vehicle
                <select
                  className={field}
                  value={vehicle}
                  onChange={(e) =>
                    setVehicle(e.target.value as Expense["vehicle"])
                  }
                >
                  <option value="none">Not a vehicle cost</option>
                  <option value="company">Company vehicle</option>
                  <option value="personal">Personal vehicle</option>
                </select>
              </label>
            </div>
          )}

          <div className="grid gap-3 sm:grid-cols-3">
            <label className={label}>
              Property
              <select
                className={field}
                value={propertyId}
                onChange={(e) => {
                  setPropertyId(e.target.value);
                  setTicketId("");
                  setProjectId("");
                  if (!e.target.value) setProjects([]);
                }}
              >
                <option value="">None</option>
                {properties.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </select>
            </label>
            <label className={label}>
              Work order
              <select
                className={field}
                value={ticketId}
                onChange={(e) => {
                  setTicketId(e.target.value);
                  const t = tickets.find((x) => x.id === e.target.value);
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
            </label>
            <label className={label}>
              Rehab project
              <select
                className={field}
                value={projectId}
                disabled={!propertyId && !projectId}
                onChange={(e) => setProjectId(e.target.value)}
              >
                <option value="">
                  {propertyId ? "None" : "Pick a property first"}
                </option>
                {projectId &&
                  !projects.some((p) => p.id === projectId) &&
                  initial?.project_name && (
                    <option value={projectId}>{initial.project_name}</option>
                  )}
                {projects.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </select>
            </label>
          </div>

          <div className="flex flex-wrap gap-x-5 gap-y-2 text-sm text-ink-2">
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={taxDeductible}
                onChange={(e) => setTaxDeductible(e.target.checked)}
              />
              Tax deductible
            </label>
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={reimbursable}
                onChange={(e) => setReimbursable(e.target.checked)}
              />
              Someone paid out of pocket (pay them back)
            </label>
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={billable}
                onChange={(e) => setBillable(e.target.checked)}
              />
              Bill it to the owner
            </label>
          </div>

          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" disabled={busy}>
              {busy
                ? "Saving…"
                : initial
                  ? "Save"
                  : isMileage
                    ? "Log trip"
                    : "Add expense"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

// ---------------------------------------------------------------------------
// Receipts
// ---------------------------------------------------------------------------

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
  const [list, setList] = useState<Receipt[] | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    expenses
      .receipts(expense.id)
      .then(setList)
      .catch((e) => toast.error(errMsg(e, "Couldn't load receipts")));
  }, [expense.id]);

  useEffect(() => {
    load();
  }, [load]);

  async function upload(file: File) {
    setBusy(true);
    try {
      await expenses.uploadReceipt(expense.id, file);
      toast.success("Receipt added");
      load();
      onChanged();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't upload the receipt"));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Receipts</DialogTitle>
          <DialogDescription>
            {expense.vendor || expense.description || "Expense"} ·{" "}
            {expense.incurred_on} · {money(expense.amount_cents)}
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-2 text-sm">
          {list === null && <p className="text-ink-3">Loading…</p>}
          {list?.length === 0 && (
            <p className="text-ink-3">No receipts on this expense yet.</p>
          )}
          {list?.map((r) => (
            <a
              key={r.document_id}
              href={r.download_url}
              target="_blank"
              rel="noopener noreferrer"
              className="flex items-center justify-between gap-3 rounded-xl border border-line px-3 py-2 hover:border-accent"
            >
              <span className="min-w-0 truncate font-semibold">
                {r.filename}
              </span>
              <span className="shrink-0 text-xs text-ink-3">
                {r.created_at.slice(0, 10)}
              </span>
            </a>
          ))}
        </div>
        <DialogFooter>
          {manage && (
            <label
              className={`inline-flex cursor-pointer items-center justify-center gap-2 rounded-xl bg-accent px-4 py-2.5 text-sm font-bold text-on-accent hover:opacity-90 ${busy ? "pointer-events-none opacity-50" : ""}`}
            >
              {busy ? "Uploading…" : "Add receipt"}
              <input
                type="file"
                accept="image/*,application/pdf"
                className="hidden"
                onChange={(e) => {
                  const f = e.target.files?.[0];
                  e.target.value = "";
                  if (f) void upload(f);
                }}
              />
            </label>
          )}
          <Button variant="outline" onClick={onClose}>
            Close
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
