"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import {
  Building2,
  DoorOpen,
  Landmark,
  Plus,
  Wallet,
} from "lucide-react";

import { useAuth } from "@/lib/auth";
import { useLlcGroups, useCreateLlc } from "@/lib/queries";
import { currencyFromCents } from "@/lib/format";
import type { LlcGroup } from "@/lib/types";

import { PageHeader, StatCard, EmptyState } from "@/components/ui/page";
import { DataTable, type ColumnDef } from "@/components/ui/data-table";
import { Button } from "@/components/ui/button";
import { TextField, SelectField } from "@/components/ui/form-field";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";
import { Badge } from "@/components/ui";

const US_STATES = [
  "AL", "AK", "AZ", "AR", "CA", "CO", "CT", "DE", "FL", "GA",
  "HI", "ID", "IL", "IN", "IA", "KS", "KY", "LA", "ME", "MD",
  "MA", "MI", "MN", "MS", "MO", "MT", "NE", "NV", "NH", "NJ",
  "NM", "NY", "NC", "ND", "OH", "OK", "OR", "PA", "RI", "SC",
  "SD", "TN", "TX", "UT", "VT", "VA", "WA", "WV", "WI", "WY", "DC",
];

const ENTITY_TYPES = [
  { value: "llc", label: "LLC" },
  { value: "series_llc", label: "Series LLC" },
  { value: "corporation", label: "Corporation" },
  { value: "s_corp", label: "S Corporation" },
  { value: "partnership", label: "Partnership" },
  { value: "trust", label: "Trust" },
  { value: "sole_proprietor", label: "Sole Proprietor" },
];

const createSchema = z.object({
  name: z.string().trim().min(1, "Name is required"),
  ein: z.string().trim().optional(),
  state: z.string().trim().optional(),
  entity_type: z.string().trim().optional(),
});

type CreateForm = z.infer<typeof createSchema>;

export default function LlcsPage() {
  const router = useRouter();
  const { can } = useAuth();
  const canManage = can("llc:manage");

  const groups = useLlcGroups();
  const data = groups.data ?? [];

  const [open, setOpen] = useState(false);

  const totals = data.reduce(
    (acc, g) => {
      acc.properties += g.property_count;
      acc.units += g.units;
      acc.rentCents += g.monthly_rent_cents;
      return acc;
    },
    { properties: 0, units: 0, rentCents: 0 }
  );

  const columns: ColumnDef<LlcGroup>[] = [
    {
      accessorKey: "name",
      header: "Entity",
      cell: ({ row }) => {
        const g = row.original;
        return (
          <div className="min-w-0">
            <div className="font-medium text-ink">{g.name}</div>
            {g.ein ? (
              <div data-numeric className="text-xs text-ink-3">
                EIN {g.ein}
              </div>
            ) : null}
          </div>
        );
      },
    },
    {
      accessorKey: "state",
      header: "State",
      cell: ({ row }) =>
        row.original.state ? (
          <Badge tone="neutral">{row.original.state}</Badge>
        ) : (
          <span className="text-ink-3">—</span>
        ),
    },
    {
      accessorKey: "property_count",
      header: "Properties",
      cell: ({ row }) => (
        <span data-numeric className="text-ink">
          {row.original.property_count}
        </span>
      ),
    },
    {
      accessorKey: "units",
      header: "Units",
      cell: ({ row }) => (
        <span data-numeric className="text-ink">
          {row.original.units}
        </span>
      ),
    },
    {
      accessorKey: "monthly_rent_cents",
      header: "Monthly rent",
      cell: ({ row }) => (
        <span data-numeric className="font-medium text-ink">
          {row.original.monthly_rent_label}
        </span>
      ),
    },
  ];

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Portfolio"
        title="LLCs & holding entities"
        description="Your holding companies and the assets they own."
        actions={
          canManage ? (
            <Button onClick={() => setOpen(true)}>
              <Plus className="h-4 w-4" />
              New LLC
            </Button>
          ) : undefined
        }
      />

      {/* Rollup KPIs */}
      <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
        {groups.isLoading ? (
          Array.from({ length: 4 }).map((_, i) => (
            <div key={i} className="skeleton h-[104px] rounded-xl" />
          ))
        ) : (
          <>
            <StatCard
              label="Entities"
              value={data.length}
              sub="Holding companies"
              icon={Landmark}
            />
            <StatCard
              label="Properties"
              value={totals.properties}
              sub="Owned across entities"
              icon={Building2}
            />
            <StatCard
              label="Units"
              value={totals.units}
              sub="Total doors"
              icon={DoorOpen}
            />
            <StatCard
              label="Monthly rent"
              value={currencyFromCents(totals.rentCents)}
              sub="Gross scheduled"
              icon={Wallet}
              tone="good"
            />
          </>
        )}
      </div>

      {/* Entities table / empty state */}
      {!groups.isLoading && data.length === 0 ? (
        <EmptyState
          icon={Landmark}
          title="No holding entities yet"
          description="Set up an LLC or holding company to organize ownership, generate branded documents, and roll up your portfolio."
          action={
            canManage ? (
              <Button onClick={() => setOpen(true)}>
                <Plus className="h-4 w-4" />
                New LLC
              </Button>
            ) : undefined
          }
        />
      ) : (
        <DataTable
          columns={columns}
          data={data}
          isLoading={groups.isLoading}
          onRowClick={(g) => router.push(`/console/llcs/${g.id}`)}
          searchPlaceholder="Search entities…"
          enableSearch={data.length > 8}
        />
      )}

      {canManage && (
        <CreateLlcDialog
          open={open}
          onOpenChange={setOpen}
          onCreated={(id) => router.push(`/console/llcs/${id}`)}
        />
      )}
    </div>
  );
}

function CreateLlcDialog({
  open,
  onOpenChange,
  onCreated,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCreated: (id: string) => void;
}) {
  const createLlc = useCreateLlc();

  const {
    register,
    handleSubmit,
    reset,
    formState: { errors },
  } = useForm<CreateForm>({
    resolver: zodResolver(createSchema),
    defaultValues: { name: "", ein: "", state: "", entity_type: "llc" },
  });

  function close(next: boolean) {
    if (!next) reset();
    onOpenChange(next);
  }

  const onSubmit = handleSubmit((values) => {
    createLlc.mutate(
      {
        name: values.name,
        ein: values.ein || undefined,
        state: values.state || undefined,
        entity_type: values.entity_type || undefined,
      },
      {
        onSuccess: (llc) => {
          reset();
          onOpenChange(false);
          onCreated(llc.id);
        },
      }
    );
  });

  return (
    <Dialog open={open} onOpenChange={close}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>New holding entity</DialogTitle>
          <DialogDescription>
            Create an LLC or holding company. You can add documents, branding,
            and templates after it&apos;s created.
          </DialogDescription>
        </DialogHeader>

        <form onSubmit={onSubmit} className="space-y-4">
          <TextField
            id="llc-name"
            label="Legal name"
            required
            placeholder="Maple Holdings LLC"
            error={errors.name?.message}
            {...register("name")}
          />

          <div className="grid grid-cols-2 gap-4">
            <SelectField
              id="llc-entity-type"
              label="Entity type"
              {...register("entity_type")}
            >
              {ENTITY_TYPES.map((t) => (
                <option key={t.value} value={t.value}>
                  {t.label}
                </option>
              ))}
            </SelectField>

            <SelectField id="llc-state" label="State" {...register("state")}>
              <option value="">—</option>
              {US_STATES.map((s) => (
                <option key={s} value={s}>
                  {s}
                </option>
              ))}
            </SelectField>
          </div>

          <TextField
            id="llc-ein"
            label="EIN"
            placeholder="12-3456789"
            hint="Optional — the entity's federal tax ID."
            error={errors.ein?.message}
            {...register("ein")}
          />

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => close(false)}
              disabled={createLlc.isPending}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={createLlc.isPending}>
              {createLlc.isPending ? "Creating…" : "Create LLC"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
