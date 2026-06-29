"use client";

// Property detail — the richest console surface. A breadcrumbed, tabbed profile
// over the full property graph: financials, units/leases, maintenance, title,
// financing, and enrichment intelligence. All reads come from the typed hooks in
// queries.ts; mutating actions are gated by permission and confirmed via toasts.

import * as React from "react";
import { useState } from "react";
import Link from "next/link";
import { useParams } from "next/navigation";
import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import { z } from "zod";
import {
  Banknote,
  Building2,
  DoorOpen,
  FileText,
  Gauge,
  Landmark,
  Plus,
  ScrollText,
  Sparkles,
  Trash2,
  Wrench,
} from "lucide-react";

import { useAuth } from "@/lib/auth";
import {
  useAdvanceWorkflow,
  useCreateLease,
  useCreateLien,
  useCreateMortgage,
  useCreateOwnership,
  useCreateTicket,
  useCreateUnit,
  useDeleteMortgage,
  useEnrichProperty,
  useLiens,
  useMortgages,
  useOwnership,
  useProperty,
  usePropertyEnrichment,
  usePropertyIntel,
  usePropertyLeases,
  usePropertyTickets,
  useUnits,
  useWorkflow,
} from "@/lib/queries";
import type {
  CostLine,
  EnrichmentRun,
  Lease,
  Lien,
  MaintenanceTicket,
  Mortgage,
  Ownership,
  PropertyProfile,
  Unit,
} from "@/lib/types";
import { Badge, statusTone } from "@/components/ui";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Breadcrumbs } from "@/components/ui/breadcrumbs";
import { PageHeader, StatCard, EmptyState } from "@/components/ui/page";
import { DataTable, type ColumnDef } from "@/components/ui/data-table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import {
  SelectField,
  TextareaField,
  TextField,
} from "@/components/ui/form-field";
import { formatDate, formatDateTime } from "@/lib/format";

// ----------------------------------------------------------------------------
// helpers
// ----------------------------------------------------------------------------

/** snake/lower → human label, e.g. `single_family` → `Single family`. */
function humanize(key: string | null | undefined): string {
  if (!key) return "—";
  const s = key.replace(/_/g, " ");
  return s.charAt(0).toUpperCase() + s.slice(1);
}

/** Dollars typed by a user → integer cents (or undefined when blank). */
function dollarsToCents(value: string): number | undefined {
  const n = Number(value);
  if (!value.trim() || !Number.isFinite(n)) return undefined;
  return Math.round(n * 100);
}

function leasePaymentTone(status: string): "good" | "warn" | "bad" | "neutral" {
  if (status === "current") return "good";
  if (status === "partial") return "warn";
  if (status === "late") return "bad";
  return "neutral";
}

function priorityTone(p: string): "bad" | "warn" | "info" | "neutral" {
  if (p === "urgent") return "bad";
  if (p === "high") return "warn";
  if (p === "normal") return "info";
  return "neutral";
}

function ticketStatusTone(s: string): "good" | "warn" | "info" | "neutral" {
  if (s === "resolved" || s === "closed") return "good";
  if (s === "in_progress" || s === "scheduled") return "info";
  if (s === "on_hold") return "warn";
  return "neutral";
}

/** A run is still in-flight if any enrichment run hasn't terminated. */
function hasPendingRun(runs: EnrichmentRun[] | undefined): boolean {
  return !!runs?.some(
    (r) => r.status === "pending" || r.status === "running" || r.status === "queued"
  );
}

// ----------------------------------------------------------------------------
// page
// ----------------------------------------------------------------------------

export default function PropertyDetailPage() {
  const { id } = useParams<{ id: string }>();
  const { can } = useAuth();

  const property = useProperty(id);
  const enrich = useEnrichProperty(id);

  const p = property.data;

  if (property.isLoading) {
    return (
      <div className="space-y-6">
        <div className="skeleton h-5 w-48 rounded-lg" />
        <div className="skeleton h-16 w-full rounded-xl" />
        <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
          {Array.from({ length: 4 }).map((_, i) => (
            <div key={i} className="skeleton h-[104px] rounded-xl" />
          ))}
        </div>
        <div className="skeleton h-64 w-full rounded-xl" />
      </div>
    );
  }

  if (property.isError || !p) {
    return (
      <div className="space-y-6">
        <Breadcrumbs
          items={[
            { label: "Properties", href: "/console/properties" },
            { label: "Not found" },
          ]}
        />
        <EmptyState
          icon={Building2}
          title="Couldn't load this property"
          description={
            property.error instanceof Error
              ? property.error.message
              : "The property may have been removed, or you don't have access to it."
          }
          action={
            <Button asChild variant="outline">
              <Link href="/console/properties">Back to properties</Link>
            </Button>
          }
        />
      </div>
    );
  }

  const canWrite = can("property:write");
  const stats = kpiStats(p);

  return (
    <div className="space-y-6">
      <Breadcrumbs
        items={[
          { label: "Properties", href: "/console/properties" },
          { label: p.name },
        ]}
      />

      <PageHeader
        eyebrow={humanize(p.property_type)}
        title={p.name}
        description={`${p.address}${p.city ? ` · ${p.city}` : ""}`}
        actions={
          <>
            <Badge tone={statusTone(p.status)}>{p.status}</Badge>
            {canWrite && (
              <Button
                onClick={() => enrich.mutate()}
                disabled={enrich.isPending}
              >
                <Sparkles className="h-4 w-4" />
                {enrich.isPending ? "Enriching…" : "Enrich data"}
              </Button>
            )}
          </>
        }
      />

      {/* KPI row */}
      <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
        {stats.map((s) => (
          <StatCard
            key={s.label}
            label={s.label}
            value={s.value}
            sub={s.sub}
            icon={s.icon}
            tone={s.tone}
          />
        ))}
      </div>

      <Tabs defaultValue="overview" className="space-y-0">
        <TabsList className="w-full overflow-x-auto">
          <TabsTrigger value="overview">Overview</TabsTrigger>
          <TabsTrigger value="units">Units &amp; Leases</TabsTrigger>
          <TabsTrigger value="maintenance">Maintenance</TabsTrigger>
          <TabsTrigger value="title">Title</TabsTrigger>
          <TabsTrigger value="financing">Financing</TabsTrigger>
          <TabsTrigger value="intelligence">Intelligence</TabsTrigger>
        </TabsList>

        <TabsContent value="overview">
          <OverviewTab id={id} property={p} canWrite={canWrite} />
        </TabsContent>
        <TabsContent value="units">
          <UnitsLeasesTab id={id} can={can} />
        </TabsContent>
        <TabsContent value="maintenance">
          <MaintenanceTab id={id} can={can} />
        </TabsContent>
        <TabsContent value="title">
          <TitleTab id={id} can={can} />
        </TabsContent>
        <TabsContent value="financing">
          <FinancingTab id={id} property={p} can={can} />
        </TabsContent>
        <TabsContent value="intelligence">
          <IntelligenceTab id={id} />
        </TabsContent>
      </Tabs>
    </div>
  );
}

type StatTone = "neutral" | "good" | "warn" | "bad" | "accent";

/** Build the StatCard row from KPIs, falling back to the profile labels. */
function kpiStats(p: PropertyProfile): {
  label: string;
  value: string;
  sub?: string;
  icon: typeof Banknote;
  tone: StatTone;
}[] {
  const occPct = p.units ? Math.round((p.occupied_units / p.units) * 100) : 0;
  return [
    {
      label: "Net revenue",
      value: p.net_revenue_label,
      sub: "Monthly, after costs",
      icon: Banknote,
      tone: "good",
    },
    {
      label: "Occupancy",
      value: p.occupancy,
      sub: `${occPct}% leased`,
      icon: Gauge,
      tone: occPct >= 90 ? "good" : occPct >= 75 ? "warn" : "bad",
    },
    {
      label: "Cash flow",
      value: p.cash_flow_label,
      sub: `Debt service ${p.debt_service_label}`,
      icon: DoorOpen,
      tone: p.cash_flow_cents >= 0 ? "good" : "bad",
    },
    {
      label: "Equity",
      value: p.equity_label,
      sub: p.financed ? `Loan ${p.total_loan_balance_label}` : "Unencumbered",
      icon: Landmark,
      tone: "accent",
    },
  ];
}

// ----------------------------------------------------------------------------
// shared dialog scaffold
// ----------------------------------------------------------------------------

function FormDialog({
  open,
  onOpenChange,
  trigger,
  title,
  description,
  children,
  onSubmit,
  submitLabel,
  isPending,
}: {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  trigger: React.ReactNode;
  title: string;
  description?: string;
  children: React.ReactNode;
  onSubmit: React.FormEventHandler<HTMLFormElement>;
  submitLabel: string;
  isPending: boolean;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogTrigger asChild>{trigger}</DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          {description && (
            <CardDescription>{description}</CardDescription>
          )}
        </DialogHeader>
        <form onSubmit={onSubmit} className="space-y-4">
          <div className="space-y-4">{children}</div>
          <DialogFooter>
            <Button
              type="button"
              variant="ghost"
              onClick={() => onOpenChange(false)}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={isPending}>
              {isPending ? "Saving…" : submitLabel}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

/** Simple two-column key/value row for definition lists. */
function DetailRow({
  label,
  value,
  mono,
}: {
  label: string;
  value: React.ReactNode;
  mono?: boolean;
}) {
  return (
    <div className="flex items-center justify-between gap-4 border-b border-line py-2.5 last:border-0">
      <dt className="text-sm text-ink-3">{label}</dt>
      <dd
        data-numeric={mono ? "" : undefined}
        className={
          mono
            ? "font-mono text-sm font-semibold text-ink"
            : "text-sm font-semibold text-ink"
        }
      >
        {value}
      </dd>
    </div>
  );
}

function SectionEmpty({ children }: { children: React.ReactNode }) {
  return <p className="text-sm text-ink-3">{children}</p>;
}

// ----------------------------------------------------------------------------
// Overview tab
// ----------------------------------------------------------------------------

function OverviewTab({
  id,
  property: p,
  canWrite,
}: {
  id: string;
  property: PropertyProfile;
  canWrite: boolean;
}) {
  const workflow = useWorkflow(id);
  const advance = useAdvanceWorkflow(id);
  const wf = workflow.data;

  return (
    <div className="space-y-6">
      <div className="grid gap-6 lg:grid-cols-[1.4fr_1fr]">
        {/* Cost breakdown */}
        <Card>
          <CardHeader>
            <CardTitle>Monthly cost &amp; revenue</CardTitle>
          </CardHeader>
          <CardContent className="space-y-1">
            {p.cost_breakdown.map((line: CostLine) => (
              <div
                key={line.label}
                className="flex items-center justify-between gap-4 border-b border-line py-2.5 last:border-0"
              >
                <span className="text-sm text-ink-2">{line.label}</span>
                <span
                  data-numeric
                  className={`font-mono text-sm ${
                    line.amount_cents >= 0 ? "text-good" : "text-ink"
                  }`}
                >
                  {line.amount_cents >= 0 ? "+" : "−"}
                  {line.amount_label.replace("-", "")}
                </span>
              </div>
            ))}
            <div className="flex items-center justify-between pt-3">
              <span className="font-semibold text-ink">Net revenue</span>
              <span
                data-numeric
                className="font-mono text-lg font-bold text-good"
              >
                {p.net_revenue_label}
              </span>
            </div>
          </CardContent>
        </Card>

        {/* Property details */}
        <Card>
          <CardHeader>
            <CardTitle>Details</CardTitle>
          </CardHeader>
          <CardContent className="py-1">
            <dl>
              <DetailRow label="Units" value={p.units} mono />
              <DetailRow label="Occupancy" value={p.occupancy} mono />
              <DetailRow
                label="Monthly rent"
                value={`${p.monthly_rent_label}/mo`}
                mono
              />
              <DetailRow label="Year built" value={p.year_built} mono />
              <DetailRow label="Manager" value={p.manager} />
              <DetailRow label="Strategy" value={humanize(p.strategy)} />
              <DetailRow label="Type" value={humanize(p.property_type)} />
            </dl>
          </CardContent>
        </Card>
      </div>

      {/* Financing summary */}
      <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
        <StatCard label="Net revenue" value={p.net_revenue_label} tone="good" />
        <StatCard label="Debt service" value={p.debt_service_label} />
        <StatCard
          label="Loan balance"
          value={p.total_loan_balance_label}
          tone={p.total_loan_balance_cents > 0 ? "warn" : "neutral"}
        />
        <StatCard label="Equity" value={p.equity_label} tone="accent" />
      </div>

      {/* Investment workflow */}
      <Card>
        <CardHeader>
          <div>
            <CardTitle>{wf?.strategy_label || "Investment workflow"}</CardTitle>
            {wf?.strategy_description && (
              <CardDescription className="mt-1">
                {wf.strategy_description}
              </CardDescription>
            )}
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          {workflow.isLoading ? (
            <div className="skeleton h-9 w-full rounded-lg" />
          ) : wf && wf.stages.length > 0 ? (
            <>
              <div className="flex flex-wrap gap-2">
                {wf.stages.map((s) => {
                  const active = s.current;
                  const reached = s.reached;
                  const className = active
                    ? "bg-accent text-on-accent"
                    : reached
                      ? "bg-good-soft text-good"
                      : "bg-surface-2 text-ink-2 hover:bg-surface-2/70";
                  return (
                    <button
                      key={s.key}
                      type="button"
                      disabled={!canWrite || advance.isPending || active}
                      onClick={() => advance.mutate({ to_stage: s.key })}
                      className={`rounded-full px-3.5 py-1.5 text-xs font-bold transition disabled:cursor-default disabled:opacity-100 ${className} ${
                        canWrite && !active
                          ? "cursor-pointer"
                          : "cursor-default"
                      }`}
                      title={
                        active
                          ? "Current stage"
                          : canWrite
                            ? `Move to ${s.label}`
                            : s.label
                      }
                    >
                      {reached && !active ? "✓ " : ""}
                      {s.label}
                    </button>
                  );
                })}
              </div>
              {wf.history.length > 0 && (
                <div className="space-y-1.5 border-t border-line pt-3 text-xs text-ink-3">
                  {wf.history.slice(0, 5).map((h) => (
                    <div key={h.id} className="flex items-center gap-2">
                      <span className="font-semibold text-ink-2">
                        {humanize(h.to_stage)}
                      </span>
                      {h.from_stage && <span>← {humanize(h.from_stage)}</span>}
                      <span data-numeric className="ml-auto font-mono">
                        {formatDateTime(h.created_at)}
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </>
          ) : (
            <SectionEmpty>No workflow configured for this strategy.</SectionEmpty>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

// ----------------------------------------------------------------------------
// Units & Leases tab
// ----------------------------------------------------------------------------

const unitSchema = z.object({
  unit_number: z.string().min(1, "Required"),
  beds: z.string().optional(),
  baths: z.string().optional(),
  sqft: z.string().optional(),
  market_rent: z.string().optional(),
  status: z.string().optional(),
});
type UnitForm = z.infer<typeof unitSchema>;

const leaseSchema = z.object({
  tenant_name: z.string().min(1, "Required"),
  tenant_email: z.string().email("Invalid email").optional().or(z.literal("")),
  unit_id: z.string().optional(),
  rent: z.string().min(1, "Required"),
  deposit: z.string().optional(),
  start_date: z.string().min(1, "Required"),
  end_date: z.string().optional(),
});
type LeaseForm = z.infer<typeof leaseSchema>;

function UnitsLeasesTab({
  id,
  can,
}: {
  id: string;
  can: (perm: string) => boolean;
}) {
  const units = useUnits(id);
  const leases = usePropertyLeases(id);
  const createUnit = useCreateUnit(id);
  const createLease = useCreateLease(id);
  const manage = can("lease:manage");

  const [unitOpen, setUnitOpen] = useState(false);
  const [leaseOpen, setLeaseOpen] = useState(false);

  const unitForm = useForm<UnitForm>({
    resolver: zodResolver(unitSchema),
    defaultValues: { status: "vacant" },
  });
  const leaseForm = useForm<LeaseForm>({
    resolver: zodResolver(leaseSchema),
  });

  const unitOptions = units.data ?? [];

  const submitUnit = unitForm.handleSubmit((v) => {
    createUnit.mutate(
      {
        unit_number: v.unit_number,
        beds: v.beds ? Number(v.beds) : undefined,
        baths: v.baths ? Number(v.baths) : undefined,
        sqft: v.sqft ? Number(v.sqft) : undefined,
        market_rent_cents: v.market_rent
          ? dollarsToCents(v.market_rent)
          : undefined,
        status: v.status || undefined,
      },
      {
        onSuccess: () => {
          setUnitOpen(false);
          unitForm.reset({ status: "vacant" });
        },
      }
    );
  });

  const submitLease = leaseForm.handleSubmit((v) => {
    const rentCents = dollarsToCents(v.rent);
    if (rentCents == null) {
      leaseForm.setError("rent", { message: "Enter an amount" });
      return;
    }
    createLease.mutate(
      {
        tenant_name: v.tenant_name,
        tenant_email: v.tenant_email || undefined,
        unit_id: v.unit_id || undefined,
        rent_cents: rentCents,
        deposit_cents: v.deposit ? dollarsToCents(v.deposit) : undefined,
        start_date: v.start_date,
        end_date: v.end_date || undefined,
      },
      {
        onSuccess: () => {
          setLeaseOpen(false);
          leaseForm.reset();
        },
      }
    );
  });

  const unitColumns: ColumnDef<Unit>[] = [
    {
      accessorKey: "unit_number",
      header: "Unit",
      cell: ({ row }) => (
        <span className="font-semibold text-ink">
          {row.original.unit_number}
        </span>
      ),
    },
    {
      id: "layout",
      header: "Layout",
      cell: ({ row }) => (
        <span className="text-ink-2">
          {row.original.beds ?? "—"} bd / {row.original.baths ?? "—"} ba
          {row.original.sqft ? ` · ${row.original.sqft.toLocaleString()} sqft` : ""}
        </span>
      ),
    },
    {
      id: "rent",
      header: "Market rent",
      cell: ({ row }) => (
        <span data-numeric className="font-mono text-ink-2">
          {row.original.market_rent_label ?? "—"}
        </span>
      ),
    },
    {
      accessorKey: "status",
      header: "Status",
      cell: ({ row }) => (
        <Badge tone={row.original.status === "occupied" ? "good" : "warn"}>
          {humanize(row.original.status)}
        </Badge>
      ),
    },
  ];

  const leaseColumns: ColumnDef<Lease>[] = [
    {
      accessorKey: "tenant_name",
      header: "Tenant",
      cell: ({ row }) => (
        <div className="min-w-0">
          <div className="truncate font-semibold text-ink">
            {row.original.tenant_name}
          </div>
          {row.original.tenant_email && (
            <div className="truncate text-xs text-ink-3">
              {row.original.tenant_email}
            </div>
          )}
        </div>
      ),
    },
    {
      id: "rent",
      header: "Rent",
      cell: ({ row }) => (
        <span data-numeric className="font-mono text-ink-2">
          {row.original.rent_label}/mo
        </span>
      ),
    },
    {
      accessorKey: "start_date",
      header: "Since",
      cell: ({ row }) => (
        <span data-numeric className="text-ink-2">
          {formatDate(row.original.start_date)}
        </span>
      ),
    },
    {
      accessorKey: "status",
      header: "Status",
      cell: ({ row }) => (
        <Badge tone="neutral">{humanize(row.original.status)}</Badge>
      ),
    },
    {
      accessorKey: "payment_status",
      header: "Payment",
      cell: ({ row }) => (
        <Badge tone={leasePaymentTone(row.original.payment_status)}>
          {humanize(row.original.payment_status)}
        </Badge>
      ),
    },
  ];

  return (
    <div className="space-y-6">
      <Card>
        <CardHeader>
          <CardTitle>Units</CardTitle>
          {manage && (
            <FormDialog
              open={unitOpen}
              onOpenChange={setUnitOpen}
              trigger={
                <Button size="sm" variant="outline">
                  <Plus className="h-4 w-4" />
                  Add unit
                </Button>
              }
              title="Add unit"
              description="Create a rentable unit on this property."
              onSubmit={submitUnit}
              submitLabel="Add unit"
              isPending={createUnit.isPending}
            >
              <TextField
                label="Unit number"
                required
                error={unitForm.formState.errors.unit_number?.message}
                {...unitForm.register("unit_number")}
              />
              <div className="grid grid-cols-3 gap-3">
                <TextField
                  label="Beds"
                  type="number"
                  {...unitForm.register("beds")}
                />
                <TextField
                  label="Baths"
                  type="number"
                  step="0.5"
                  {...unitForm.register("baths")}
                />
                <TextField
                  label="Sqft"
                  type="number"
                  {...unitForm.register("sqft")}
                />
              </div>
              <div className="grid grid-cols-2 gap-3">
                <TextField
                  label="Market rent ($/mo)"
                  type="number"
                  {...unitForm.register("market_rent")}
                />
                <SelectField label="Status" {...unitForm.register("status")}>
                  <option value="vacant">Vacant</option>
                  <option value="occupied">Occupied</option>
                  <option value="turn">Turn</option>
                  <option value="offline">Offline</option>
                </SelectField>
              </div>
            </FormDialog>
          )}
        </CardHeader>
        <CardContent>
          <DataTable
            columns={unitColumns}
            data={unitOptions}
            isLoading={units.isLoading}
            enableSearch={false}
            pageSize={8}
            emptyState={
              <div className="py-8">
                <SectionEmpty>No units recorded for this property.</SectionEmpty>
              </div>
            }
          />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Leases</CardTitle>
          {manage && (
            <FormDialog
              open={leaseOpen}
              onOpenChange={setLeaseOpen}
              trigger={
                <Button size="sm" variant="outline">
                  <Plus className="h-4 w-4" />
                  New lease
                </Button>
              }
              title="New lease"
              description="Start a tenancy on this property."
              onSubmit={submitLease}
              submitLabel="Create lease"
              isPending={createLease.isPending}
            >
              <TextField
                label="Tenant name"
                required
                error={leaseForm.formState.errors.tenant_name?.message}
                {...leaseForm.register("tenant_name")}
              />
              <TextField
                label="Tenant email"
                type="email"
                error={leaseForm.formState.errors.tenant_email?.message}
                {...leaseForm.register("tenant_email")}
              />
              <SelectField
                label="Unit"
                hint="Optional — leave blank for a whole-property lease."
                {...leaseForm.register("unit_id")}
              >
                <option value="">No specific unit</option>
                {unitOptions.map((u) => (
                  <option key={u.id} value={u.id}>
                    Unit {u.unit_number}
                  </option>
                ))}
              </SelectField>
              <div className="grid grid-cols-2 gap-3">
                <TextField
                  label="Rent ($/mo)"
                  type="number"
                  required
                  error={leaseForm.formState.errors.rent?.message}
                  {...leaseForm.register("rent")}
                />
                <TextField
                  label="Deposit ($)"
                  type="number"
                  {...leaseForm.register("deposit")}
                />
              </div>
              <div className="grid grid-cols-2 gap-3">
                <TextField
                  label="Start date"
                  type="date"
                  required
                  error={leaseForm.formState.errors.start_date?.message}
                  {...leaseForm.register("start_date")}
                />
                <TextField
                  label="End date"
                  type="date"
                  {...leaseForm.register("end_date")}
                />
              </div>
            </FormDialog>
          )}
        </CardHeader>
        <CardContent>
          <DataTable
            columns={leaseColumns}
            data={leases.data ?? []}
            isLoading={leases.isLoading}
            enableSearch={false}
            pageSize={8}
            emptyState={
              <div className="py-8">
                <SectionEmpty>No leases recorded for this property.</SectionEmpty>
              </div>
            }
          />
        </CardContent>
      </Card>
    </div>
  );
}

// ----------------------------------------------------------------------------
// Maintenance tab
// ----------------------------------------------------------------------------

const ticketSchema = z.object({
  title: z.string().min(1, "Required"),
  category: z.string().optional(),
  priority: z.string().optional(),
  reporter: z.string().optional(),
  description: z.string().optional(),
});
type TicketForm = z.infer<typeof ticketSchema>;

function MaintenanceTab({
  id,
  can,
}: {
  id: string;
  can: (perm: string) => boolean;
}) {
  const tickets = usePropertyTickets(id);
  const createTicket = useCreateTicket(id);
  const manage = can("maintenance:manage");
  const [open, setOpen] = useState(false);

  const form = useForm<TicketForm>({
    resolver: zodResolver(ticketSchema),
    defaultValues: { category: "general", priority: "normal" },
  });

  const submit = form.handleSubmit((v) => {
    createTicket.mutate(
      {
        title: v.title,
        category: v.category || undefined,
        priority: v.priority || undefined,
        reporter: v.reporter || undefined,
        description: v.description || undefined,
      },
      {
        onSuccess: () => {
          setOpen(false);
          form.reset({ category: "general", priority: "normal" });
        },
      }
    );
  });

  const columns: ColumnDef<MaintenanceTicket>[] = [
    {
      accessorKey: "title",
      header: "Ticket",
      cell: ({ row }) => (
        <div className="min-w-0">
          <div className="truncate font-semibold text-ink">
            {row.original.title}
          </div>
          <div className="truncate text-xs text-ink-3">
            {humanize(row.original.category)}
            {row.original.reporter ? ` · ${row.original.reporter}` : ""}
          </div>
        </div>
      ),
    },
    {
      accessorKey: "priority",
      header: "Priority",
      cell: ({ row }) => (
        <Badge tone={priorityTone(row.original.priority)}>
          {humanize(row.original.priority)}
        </Badge>
      ),
    },
    {
      accessorKey: "status",
      header: "Status",
      cell: ({ row }) => (
        <Badge tone={ticketStatusTone(row.original.status)}>
          {humanize(row.original.status)}
        </Badge>
      ),
    },
    {
      id: "cost",
      header: "Cost",
      cell: ({ row }) => (
        <span data-numeric className="font-mono text-ink-2">
          {row.original.cost_label ?? "—"}
        </span>
      ),
    },
  ];

  return (
    <Card>
      <CardHeader>
        <CardTitle>Maintenance &amp; work orders</CardTitle>
        {manage && (
          <FormDialog
            open={open}
            onOpenChange={setOpen}
            trigger={
              <Button size="sm" variant="outline">
                <Plus className="h-4 w-4" />
                New ticket
              </Button>
            }
            title="New work order"
            description="Open a maintenance ticket against this property."
            onSubmit={submit}
            submitLabel="Create ticket"
            isPending={createTicket.isPending}
          >
            <TextField
              label="Title"
              required
              error={form.formState.errors.title?.message}
              {...form.register("title")}
            />
            <div className="grid grid-cols-2 gap-3">
              <SelectField label="Category" {...form.register("category")}>
                <option value="general">General</option>
                <option value="plumbing">Plumbing</option>
                <option value="electrical">Electrical</option>
                <option value="hvac">HVAC</option>
                <option value="appliance">Appliance</option>
                <option value="structural">Structural</option>
                <option value="landscaping">Landscaping</option>
                <option value="turn">Turn</option>
              </SelectField>
              <SelectField label="Priority" {...form.register("priority")}>
                <option value="low">Low</option>
                <option value="normal">Normal</option>
                <option value="high">High</option>
                <option value="urgent">Urgent</option>
              </SelectField>
            </div>
            <TextField label="Reporter" {...form.register("reporter")} />
            <TextareaField
              label="Description"
              {...form.register("description")}
            />
          </FormDialog>
        )}
      </CardHeader>
      <CardContent>
        <DataTable
          columns={columns}
          data={tickets.data ?? []}
          isLoading={tickets.isLoading}
          searchPlaceholder="Search tickets…"
          pageSize={10}
          emptyState={
            <div className="py-10">
              <EmptyState
                className="border-0 bg-transparent py-0"
                icon={Wrench}
                title="No maintenance tickets"
                description="Open work orders against this property to track repairs and turns."
              />
            </div>
          }
        />
      </CardContent>
    </Card>
  );
}

// ----------------------------------------------------------------------------
// Title tab
// ----------------------------------------------------------------------------

const ownershipSchema = z.object({
  owner_name: z.string().min(1, "Required"),
  owner_kind: z.string().optional(),
  vesting: z.string().optional(),
  percent: z.string().optional(),
  deed_recorded_date: z.string().optional(),
});
type OwnershipForm = z.infer<typeof ownershipSchema>;

const lienSchema = z.object({
  lienholder_name: z.string().min(1, "Required"),
  kind: z.string().optional(),
  amount: z.string().optional(),
  position: z.string().optional(),
  status: z.string().optional(),
  recorded_date: z.string().optional(),
});
type LienForm = z.infer<typeof lienSchema>;

function TitleTab({ id, can }: { id: string; can: (perm: string) => boolean }) {
  const ownership = useOwnership(id);
  const liens = useLiens(id);
  const createOwnership = useCreateOwnership(id);
  const createLien = useCreateLien(id);
  const manage = can("title:manage");

  const [ownerOpen, setOwnerOpen] = useState(false);
  const [lienOpen, setLienOpen] = useState(false);

  const ownerForm = useForm<OwnershipForm>({
    resolver: zodResolver(ownershipSchema),
    defaultValues: { owner_kind: "individual" },
  });
  const lienForm = useForm<LienForm>({
    resolver: zodResolver(lienSchema),
    defaultValues: { kind: "mortgage", status: "active" },
  });

  const submitOwner = ownerForm.handleSubmit((v) => {
    const pct = v.percent ? Number(v.percent) : undefined;
    createOwnership.mutate(
      {
        owner_name: v.owner_name,
        owner_kind: v.owner_kind || undefined,
        vesting: v.vesting || undefined,
        percent_bps:
          pct != null && Number.isFinite(pct)
            ? Math.round(pct * 100)
            : undefined,
        deed_recorded_date: v.deed_recorded_date || undefined,
      },
      {
        onSuccess: () => {
          setOwnerOpen(false);
          ownerForm.reset({ owner_kind: "individual" });
        },
      }
    );
  });

  const submitLien = lienForm.handleSubmit((v) => {
    createLien.mutate(
      {
        lienholder_name: v.lienholder_name,
        kind: v.kind || undefined,
        amount_cents: v.amount ? dollarsToCents(v.amount) : undefined,
        position: v.position ? Number(v.position) : undefined,
        status: v.status || undefined,
        recorded_date: v.recorded_date || undefined,
      },
      {
        onSuccess: () => {
          setLienOpen(false);
          lienForm.reset({ kind: "mortgage", status: "active" });
        },
      }
    );
  });

  return (
    <div className="grid gap-6 lg:grid-cols-2">
      {/* Ownership */}
      <Card>
        <CardHeader>
          <CardTitle>Ownership of record</CardTitle>
          {manage && (
            <FormDialog
              open={ownerOpen}
              onOpenChange={setOwnerOpen}
              trigger={
                <Button size="sm" variant="outline">
                  <Plus className="h-4 w-4" />
                  Add owner
                </Button>
              }
              title="Record ownership"
              description="Add a vesting / deed record to the title."
              onSubmit={submitOwner}
              submitLabel="Record owner"
              isPending={createOwnership.isPending}
            >
              <TextField
                label="Owner name"
                required
                error={ownerForm.formState.errors.owner_name?.message}
                {...ownerForm.register("owner_name")}
              />
              <div className="grid grid-cols-2 gap-3">
                <SelectField
                  label="Owner kind"
                  {...ownerForm.register("owner_kind")}
                >
                  <option value="individual">Individual</option>
                  <option value="llc">LLC</option>
                  <option value="trust">Trust</option>
                  <option value="entity">Entity</option>
                </SelectField>
                <TextField
                  label="Ownership %"
                  type="number"
                  step="0.01"
                  {...ownerForm.register("percent")}
                />
              </div>
              <TextField
                label="Vesting"
                hint="e.g. Joint tenants, Sole owner"
                {...ownerForm.register("vesting")}
              />
              <TextField
                label="Deed recorded"
                type="date"
                {...ownerForm.register("deed_recorded_date")}
              />
            </FormDialog>
          )}
        </CardHeader>
        <CardContent className="space-y-3">
          {ownership.isLoading ? (
            <div className="skeleton h-16 w-full rounded-lg" />
          ) : (ownership.data?.length ?? 0) === 0 ? (
            <SectionEmpty>No ownership recorded.</SectionEmpty>
          ) : (
            ownership.data!.map((o: Ownership) => (
              <div
                key={o.id}
                className="flex items-center justify-between gap-4 border-b border-line pb-3 last:border-0"
              >
                <div className="min-w-0">
                  <div className="truncate font-semibold text-ink">
                    {o.owner_name}
                  </div>
                  <div className="truncate text-xs text-ink-3">
                    {o.vesting ?? humanize(o.owner_kind)}
                    {o.deed_recorded_date
                      ? ` · ${formatDate(o.deed_recorded_date)}`
                      : ""}
                  </div>
                </div>
                <span data-numeric className="font-mono text-sm text-ink-2">
                  {(o.percent_bps / 100).toFixed(0)}%
                </span>
              </div>
            ))
          )}
        </CardContent>
      </Card>

      {/* Liens */}
      <Card>
        <CardHeader>
          <CardTitle>Liens</CardTitle>
          {manage && (
            <FormDialog
              open={lienOpen}
              onOpenChange={setLienOpen}
              trigger={
                <Button size="sm" variant="outline">
                  <Plus className="h-4 w-4" />
                  Add lien
                </Button>
              }
              title="Record lien"
              description="Add a lien recorded against this property's title."
              onSubmit={submitLien}
              submitLabel="Record lien"
              isPending={createLien.isPending}
            >
              <TextField
                label="Lienholder"
                required
                error={lienForm.formState.errors.lienholder_name?.message}
                {...lienForm.register("lienholder_name")}
              />
              <div className="grid grid-cols-2 gap-3">
                <SelectField label="Kind" {...lienForm.register("kind")}>
                  <option value="mortgage">Mortgage</option>
                  <option value="tax">Tax</option>
                  <option value="mechanic">Mechanic</option>
                  <option value="judgment">Judgment</option>
                  <option value="hoa">HOA</option>
                </SelectField>
                <SelectField label="Status" {...lienForm.register("status")}>
                  <option value="active">Active</option>
                  <option value="released">Released</option>
                  <option value="disputed">Disputed</option>
                </SelectField>
              </div>
              <div className="grid grid-cols-2 gap-3">
                <TextField
                  label="Amount ($)"
                  type="number"
                  {...lienForm.register("amount")}
                />
                <TextField
                  label="Position"
                  type="number"
                  {...lienForm.register("position")}
                />
              </div>
              <TextField
                label="Recorded date"
                type="date"
                {...lienForm.register("recorded_date")}
              />
            </FormDialog>
          )}
        </CardHeader>
        <CardContent className="space-y-3">
          {liens.isLoading ? (
            <div className="skeleton h-16 w-full rounded-lg" />
          ) : (liens.data?.length ?? 0) === 0 ? (
            <SectionEmpty>No liens recorded.</SectionEmpty>
          ) : (
            liens.data!.map((ln: Lien) => (
              <div
                key={ln.id}
                className="flex items-center justify-between gap-4 border-b border-line pb-3 last:border-0"
              >
                <div className="min-w-0">
                  <div className="truncate font-semibold text-ink">
                    {ln.lienholder_name}
                  </div>
                  <div className="truncate text-xs text-ink-3">
                    {humanize(ln.kind)}
                    {ln.position != null ? ` · lien ${ln.position}` : ""}
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  {ln.amount_label && (
                    <span data-numeric className="font-mono text-sm text-ink-2">
                      {ln.amount_label}
                    </span>
                  )}
                  <Badge tone={ln.status === "active" ? "warn" : "neutral"}>
                    {humanize(ln.status)}
                  </Badge>
                </div>
              </div>
            ))
          )}
        </CardContent>
      </Card>
    </div>
  );
}

// ----------------------------------------------------------------------------
// Financing tab
// ----------------------------------------------------------------------------

const mortgageSchema = z.object({
  kind: z.string().optional(),
  position: z.string().optional(),
  original_amount: z.string().optional(),
  current_balance: z.string().optional(),
  interest_rate: z.string().optional(),
  term_months: z.string().optional(),
  monthly_payment: z.string().optional(),
  loan_number: z.string().optional(),
});
type MortgageForm = z.infer<typeof mortgageSchema>;

function FinancingTab({
  id,
  property: p,
  can,
}: {
  id: string;
  property: PropertyProfile;
  can: (perm: string) => boolean;
}) {
  const mortgages = useMortgages(id);
  const createMortgage = useCreateMortgage(id);
  const deleteMortgage = useDeleteMortgage();
  const manage = can("finance:manage");
  const [open, setOpen] = useState(false);

  const form = useForm<MortgageForm>({
    resolver: zodResolver(mortgageSchema),
    defaultValues: { kind: "purchase", position: "1" },
  });

  const submit = form.handleSubmit((v) => {
    const ratePct = v.interest_rate ? Number(v.interest_rate) : undefined;
    createMortgage.mutate(
      {
        kind: v.kind || "purchase",
        position: v.position ? Number(v.position) : undefined,
        original_amount_cents: v.original_amount
          ? dollarsToCents(v.original_amount)
          : undefined,
        current_balance_cents: v.current_balance
          ? dollarsToCents(v.current_balance)
          : undefined,
        interest_rate_bps:
          ratePct != null && Number.isFinite(ratePct)
            ? Math.round(ratePct * 100)
            : undefined,
        term_months: v.term_months ? Number(v.term_months) : undefined,
        monthly_payment_cents: v.monthly_payment
          ? dollarsToCents(v.monthly_payment)
          : undefined,
        loan_number: v.loan_number || undefined,
      },
      {
        onSuccess: () => {
          setOpen(false);
          form.reset({ kind: "purchase", position: "1" });
        },
      }
    );
  });

  const list = mortgages.data ?? [];

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle>Financing</CardTitle>
          <div className="mt-2 flex flex-wrap gap-2">
            <Badge tone="info">Loan balance {p.total_loan_balance_label}</Badge>
            <Badge tone="good">Equity {p.equity_label}</Badge>
            <Badge tone="neutral">Debt service {p.debt_service_label}</Badge>
          </div>
        </div>
        {manage && (
          <FormDialog
            open={open}
            onOpenChange={setOpen}
            trigger={
              <Button size="sm" variant="outline">
                <Plus className="h-4 w-4" />
                Add loan
              </Button>
            }
            title="Add mortgage"
            description="Record a loan financing this property."
            onSubmit={submit}
            submitLabel="Add loan"
            isPending={createMortgage.isPending}
          >
            <div className="grid grid-cols-2 gap-3">
              <SelectField label="Kind" {...form.register("kind")}>
                <option value="purchase">Purchase</option>
                <option value="refinance">Refinance</option>
                <option value="heloc">HELOC</option>
                <option value="bridge">Bridge</option>
                <option value="construction">Construction</option>
              </SelectField>
              <TextField
                label="Lien position"
                type="number"
                {...form.register("position")}
              />
            </div>
            <div className="grid grid-cols-2 gap-3">
              <TextField
                label="Original amount ($)"
                type="number"
                {...form.register("original_amount")}
              />
              <TextField
                label="Current balance ($)"
                type="number"
                {...form.register("current_balance")}
              />
            </div>
            <div className="grid grid-cols-3 gap-3">
              <TextField
                label="Rate (%)"
                type="number"
                step="0.01"
                {...form.register("interest_rate")}
              />
              <TextField
                label="Term (mo)"
                type="number"
                {...form.register("term_months")}
              />
              <TextField
                label="Payment ($)"
                type="number"
                {...form.register("monthly_payment")}
              />
            </div>
            <TextField label="Loan number" {...form.register("loan_number")} />
          </FormDialog>
        )}
      </CardHeader>
      <CardContent className="space-y-3">
        {mortgages.isLoading ? (
          <div className="skeleton h-20 w-full rounded-lg" />
        ) : list.length === 0 ? (
          <EmptyState
            className="border-0 bg-transparent py-6"
            icon={Landmark}
            title="No loans recorded"
            description="This property is held free and clear, or financing hasn't been added yet."
          />
        ) : (
          list.map((m: Mortgage) => (
            <div
              key={m.id}
              className="flex items-center justify-between gap-4 rounded-xl border border-line bg-surface-2/40 px-4 py-3"
            >
              <div className="min-w-0">
                <div className="font-semibold text-ink">
                  {humanize(m.kind)}{" "}
                  <span className="text-xs font-normal text-ink-3">
                    · lien {m.position}
                  </span>
                  {m.loan_number && (
                    <span className="ml-2 font-mono text-xs text-ink-3">
                      {m.loan_number}
                    </span>
                  )}
                </div>
                <div className="mt-0.5 flex flex-wrap gap-x-4 text-xs text-ink-3">
                  <span data-numeric className="font-mono">
                    Balance {m.current_balance_label ?? "—"}
                  </span>
                  <span data-numeric className="font-mono">
                    Rate{" "}
                    {m.interest_rate_pct != null
                      ? `${m.interest_rate_pct.toFixed(2)}%`
                      : "—"}
                  </span>
                  <span data-numeric className="font-mono">
                    Payment {m.monthly_payment_label ?? "—"}
                  </span>
                </div>
              </div>
              {manage && (
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label="Delete loan"
                  disabled={deleteMortgage.isPending}
                  onClick={() => deleteMortgage.mutate(m.id)}
                  className="text-ink-3 hover:text-bad"
                >
                  <Trash2 className="h-4 w-4" />
                </Button>
              )}
            </div>
          ))
        )}
      </CardContent>
    </Card>
  );
}

// ----------------------------------------------------------------------------
// Intelligence tab
// ----------------------------------------------------------------------------

function IntelligenceTab({ id }: { id: string }) {
  const enrichment = usePropertyEnrichment(id);
  const pending = hasPendingRun(enrichment.data);
  // Poll while a run is in-flight so the UI fills in as jobs complete.
  const refetchInterval = pending ? 4000 : false;

  const intel = usePropertyIntel(id, { refetchInterval });
  const runs = usePropertyEnrichment(id, { refetchInterval });

  const d = intel.data?.detail ?? null;
  const latestValue = intel.data?.valuations?.[0];

  const loading = intel.isLoading || runs.isLoading;

  const hasIntel =
    !!d ||
    !!intel.data?.taxes?.length ||
    !!intel.data?.schools?.length ||
    !!intel.data?.utilities?.length;

  if (loading) {
    return (
      <div className="space-y-6">
        <div className="skeleton h-48 w-full rounded-xl" />
        <div className="grid gap-6 lg:grid-cols-2">
          <div className="skeleton h-40 rounded-xl" />
          <div className="skeleton h-40 rounded-xl" />
        </div>
      </div>
    );
  }

  if (!hasIntel) {
    return (
      <EmptyState
        icon={Sparkles}
        title="No intelligence yet"
        description='Run "Enrich data" to fetch parcel records, valuations, taxes, schools, and utilities for this property.'
      />
    );
  }

  return (
    <div className="space-y-6">
      {pending && (
        <div className="flex items-center gap-2 rounded-xl border border-line bg-info-soft px-4 py-3 text-sm text-info">
          <Sparkles className="h-4 w-4 animate-pulse" />
          Enrichment in progress — data updates automatically as jobs complete.
        </div>
      )}

      {/* Valuation + parcel */}
      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Valuation</CardTitle>
          </CardHeader>
          <CardContent className="py-1">
            {latestValue ? (
              <dl>
                <DetailRow
                  label="Estimated value (AVM)"
                  value={latestValue.estimated_value_label ?? "—"}
                  mono
                />
                {latestValue.confidence != null && (
                  <DetailRow
                    label="Confidence"
                    value={`${latestValue.confidence}%`}
                    mono
                  />
                )}
                <DetailRow
                  label="Est. market rent"
                  value={
                    latestValue.estimated_rent_label
                      ? `${latestValue.estimated_rent_label}/mo`
                      : "—"
                  }
                  mono
                />
                <DetailRow label="Source" value={humanize(latestValue.source)} />
                <DetailRow label="As of" value={formatDate(latestValue.as_of)} />
              </dl>
            ) : (
              <SectionEmpty>No valuation on file.</SectionEmpty>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Parcel &amp; county record</CardTitle>
          </CardHeader>
          <CardContent className="py-1">
            {d ? (
              <dl>
                <DetailRow label="APN" value={d.apn ?? "—"} mono />
                <DetailRow label="County" value={d.county ?? "—"} />
                <DetailRow label="Zoning" value={d.zoning ?? "—"} />
                <DetailRow
                  label="Lot size"
                  value={
                    d.lot_size_sqft
                      ? `${d.lot_size_sqft.toLocaleString()} sqft`
                      : "—"
                  }
                  mono
                />
                <DetailRow
                  label="Living area"
                  value={d.sqft ? `${d.sqft.toLocaleString()} sqft` : "—"}
                  mono
                />
                <DetailRow
                  label="Beds / baths"
                  value={`${d.beds ?? "—"} / ${d.baths ?? "—"}`}
                  mono
                />
                <DetailRow
                  label="Last sale"
                  value={
                    d.last_sale_date
                      ? `${formatDate(d.last_sale_date)}${
                          d.last_sale_price_label
                            ? ` · ${d.last_sale_price_label}`
                            : ""
                        }`
                      : "—"
                  }
                />
              </dl>
            ) : (
              <SectionEmpty>No parcel record on file.</SectionEmpty>
            )}
            {d?.last_enriched_at && (
              <p className="mt-3 text-xs text-ink-3">
                Last enriched {formatDateTime(d.last_enriched_at)}
              </p>
            )}
          </CardContent>
        </Card>
      </div>

      {/* Schools + utilities */}
      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Schools</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {intel.data?.schools?.length ? (
              intel.data.schools.map((s) => (
                <div
                  key={`${s.level}-${s.name}`}
                  className="flex items-center justify-between gap-4 border-b border-line pb-3 last:border-0"
                >
                  <div className="min-w-0">
                    <div className="truncate font-semibold text-ink">
                      {s.name}
                    </div>
                    <div className="truncate text-xs text-ink-3">
                      {humanize(s.level)}
                      {s.grades ? ` · ${s.grades}` : ""}
                      {s.distance_mi != null ? ` · ${s.distance_mi} mi` : ""}
                    </div>
                  </div>
                  {s.rating != null && (
                    <Badge
                      tone={
                        s.rating >= 8 ? "good" : s.rating >= 5 ? "warn" : "neutral"
                      }
                    >
                      {s.rating}/10
                    </Badge>
                  )}
                </div>
              ))
            ) : (
              <SectionEmpty>No school data.</SectionEmpty>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Utilities</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {intel.data?.utilities?.length ? (
              intel.data.utilities.map((u) => (
                <div
                  key={u.utility_type}
                  className="flex items-center justify-between gap-4 border-b border-line pb-3 last:border-0"
                >
                  <div className="min-w-0">
                    <div className="truncate font-semibold text-ink">
                      {humanize(u.utility_type)}
                    </div>
                    <div className="truncate text-xs text-ink-3">
                      {u.provider}
                    </div>
                  </div>
                  <span data-numeric className="font-mono text-sm text-ink-2">
                    {u.est_monthly_cost_label
                      ? `${u.est_monthly_cost_label}/mo`
                      : "—"}
                  </span>
                </div>
              ))
            ) : (
              <SectionEmpty>No utility data.</SectionEmpty>
            )}
          </CardContent>
        </Card>
      </div>

      {/* Tax history */}
      <Card>
        <CardHeader>
          <CardTitle>Tax &amp; assessment history</CardTitle>
        </CardHeader>
        <CardContent className="p-0">
          {intel.data?.taxes?.length ? (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-line text-left text-xs font-semibold uppercase tracking-wide text-ink-3">
                    <th className="px-5 py-2.5">Year</th>
                    <th className="px-5 py-2.5">Assessed value</th>
                    <th className="px-5 py-2.5 text-right">Tax</th>
                    <th className="px-5 py-2.5 text-right">Rate</th>
                  </tr>
                </thead>
                <tbody>
                  {intel.data.taxes.map((t) => (
                    <tr
                      key={t.tax_year}
                      className="border-b border-line last:border-0"
                    >
                      <td className="px-5 py-3 font-semibold text-ink">
                        {t.tax_year}
                      </td>
                      <td
                        data-numeric
                        className="px-5 py-3 font-mono text-ink-2"
                      >
                        {t.assessed_value_label ?? "—"}
                      </td>
                      <td
                        data-numeric
                        className="px-5 py-3 text-right font-mono text-ink-2"
                      >
                        {t.tax_amount_label ?? "—"}
                      </td>
                      <td
                        data-numeric
                        className="px-5 py-3 text-right font-mono text-ink-3"
                      >
                        {t.tax_rate_bps != null
                          ? `${(t.tax_rate_bps / 100).toFixed(2)}%`
                          : "—"}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : (
            <div className="p-5">
              <SectionEmpty>No tax history.</SectionEmpty>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Enrichment trail */}
      <Card>
        <CardHeader>
          <CardTitle>Enrichment activity</CardTitle>
        </CardHeader>
        <CardContent>
          {runs.data?.length ? (
            <div className="space-y-2">
              {runs.data.slice(0, 16).map((r) => (
                <div
                  key={r.id}
                  className="flex items-center justify-between gap-3 border-b border-line pb-2 text-sm last:border-0"
                >
                  <div className="flex items-center gap-2">
                    <FileText className="h-3.5 w-3.5 text-ink-3" />
                    <span className="font-medium text-ink">
                      {humanize(r.source)}
                    </span>
                    <span className="text-xs text-ink-3">{r.provider}</span>
                  </div>
                  <div className="flex items-center gap-3">
                    <Badge tone={runTone(r.status)}>{humanize(r.status)}</Badge>
                    <span data-numeric className="font-mono text-xs text-ink-3">
                      {formatDateTime(r.created_at)}
                    </span>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="flex items-center gap-2 text-sm text-ink-3">
              <ScrollText className="h-4 w-4" />
              No enrichment runs yet.
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

function runTone(status: string): "good" | "bad" | "warn" | "info" | "neutral" {
  if (status === "succeeded" || status === "complete") return "good";
  if (status === "failed" || status === "error") return "bad";
  if (status === "pending" || status === "running" || status === "queued")
    return "info";
  return "neutral";
}
