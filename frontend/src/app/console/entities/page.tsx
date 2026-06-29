"use client";

// Entities / counterparties registry. A searchable, filterable table of banks,
// lenders, contractors, insurers and other counterparties. Reads via
// useEntities(kind?); users with "entity:manage" can add new entities inline.

import { useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import {
  Building2,
  Mail,
  Phone,
  Plus,
  ShieldAlert,
  Users,
} from "lucide-react";

import { useAuth } from "@/lib/auth";
import { useCreateEntity, useEntities } from "@/lib/queries";
import type { Counterparty } from "@/lib/types";
import { titleCase } from "@/lib/format";

import { PageHeader, EmptyState } from "@/components/ui/page";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui";
import { Button } from "@/components/ui/button";
import { DataTable, type ColumnDef } from "@/components/ui/data-table";
import { TextField, TextareaField, SelectField } from "@/components/ui/form-field";
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
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

/** The counterparty kinds we offer in selects + filters, in display order. */
const KINDS = [
  "bank",
  "lender",
  "insurer",
  "title",
  "contractor",
  "inspector",
  "appraiser",
  "attorney",
  "property_manager",
  "utility",
  "other",
] as const;

/** Turn a snake/lower key into a human label, e.g. `property_manager` → `Property manager`. */
function humanize(key: string): string {
  return titleCase(key.replace(/_/g, " "));
}

const ALL = "all";

const createSchema = z.object({
  kind: z.string().min(1, "Pick a kind"),
  name: z.string().trim().min(1, "Name is required"),
  contact_name: z.string().trim().optional(),
  email: z
    .string()
    .trim()
    .email("Enter a valid email")
    .optional()
    .or(z.literal("")),
  phone: z.string().trim().optional(),
  website: z.string().trim().optional(),
  address: z.string().trim().optional(),
  notes: z.string().trim().optional(),
});

type CreateForm = z.infer<typeof createSchema>;

export default function EntitiesPage() {
  const { can } = useAuth();
  const router = useRouter();

  const canRead = can("entity:read");
  const canManage = can("entity:manage");

  const [kind, setKind] = useState<string>(ALL);
  const entities = useEntities(kind === ALL ? undefined : kind, {
    enabled: canRead,
  });

  const rows = entities.data ?? [];

  const columns = useMemo<ColumnDef<Counterparty>[]>(
    () => [
      {
        accessorKey: "name",
        header: "Name",
        cell: ({ row }) => {
          const c = row.original;
          return (
            <div className="min-w-0">
              <div className="truncate font-medium text-ink">{c.name}</div>
              {c.website && (
                <div className="truncate text-xs text-ink-3">{c.website}</div>
              )}
            </div>
          );
        },
      },
      {
        accessorKey: "kind",
        header: "Kind",
        cell: ({ row }) => (
          <Badge tone="info">{humanize(row.original.kind)}</Badge>
        ),
      },
      {
        accessorKey: "contact_name",
        header: "Contact",
        cell: ({ row }) => (
          <span className="text-ink-2">
            {row.original.contact_name ?? "—"}
          </span>
        ),
      },
      {
        accessorKey: "email",
        header: "Email",
        cell: ({ row }) =>
          row.original.email ? (
            <span className="inline-flex items-center gap-1.5 text-ink-2">
              <Mail className="h-3.5 w-3.5 text-ink-3" />
              <span className="truncate">{row.original.email}</span>
            </span>
          ) : (
            <span className="text-ink-3">—</span>
          ),
      },
      {
        accessorKey: "phone",
        header: "Phone",
        cell: ({ row }) =>
          row.original.phone ? (
            <span
              data-numeric
              className="inline-flex items-center gap-1.5 font-mono text-ink-2"
            >
              <Phone className="h-3.5 w-3.5 text-ink-3" />
              {row.original.phone}
            </span>
          ) : (
            <span className="text-ink-3">—</span>
          ),
      },
    ],
    []
  );

  if (!canRead) {
    return (
      <div className="space-y-6">
        <PageHeader
          eyebrow="Registry"
          title="Entities"
          description="Counterparties across your deals — banks, lenders, contractors and more."
        />
        <EmptyState
          icon={ShieldAlert}
          title="No access to the entities registry"
          description="Ask an admin to grant you the entity:read permission to view counterparties."
        />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Registry"
        title="Entities"
        description="Counterparties across your deals — banks, lenders, contractors and more."
        actions={canManage ? <CreateEntityDialog /> : undefined}
      />

      <Card>
        <CardContent className="space-y-4 p-4 sm:p-5">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="flex items-center gap-2">
              <span className="text-xs font-semibold uppercase tracking-wide text-ink-3">
                Kind
              </span>
              <Select value={kind} onValueChange={setKind}>
                <SelectTrigger className="w-44" aria-label="Filter by kind">
                  <SelectValue placeholder="All kinds" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value={ALL}>All kinds</SelectItem>
                  {KINDS.map((k) => (
                    <SelectItem key={k} value={k}>
                      {humanize(k)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            {!entities.isLoading && (
              <span data-numeric className="text-xs text-ink-3">
                {rows.length} {rows.length === 1 ? "entity" : "entities"}
              </span>
            )}
          </div>

          <DataTable
            columns={columns}
            data={rows}
            isLoading={entities.isLoading}
            onRowClick={(c) => router.push(`/console/entities/${c.id}`)}
            searchPlaceholder="Search entities…"
            emptyState={
              <EmptyState
                className="border-0 bg-transparent py-10"
                icon={kind === ALL ? Users : Building2}
                title={
                  kind === ALL
                    ? "No entities yet"
                    : `No ${humanize(kind).toLowerCase()} entities`
                }
                description={
                  kind === ALL
                    ? "Add banks, lenders, contractors and other counterparties to keep your deal contacts in one place."
                    : "Try a different kind, or clear the filter to see all entities."
                }
                action={canManage && kind === ALL ? <CreateEntityDialog /> : undefined}
              />
            }
          />
        </CardContent>
      </Card>
    </div>
  );
}

/** "Add entity" button + modal create form (RHF + zod), gated by the caller. */
function CreateEntityDialog() {
  const [open, setOpen] = useState(false);
  const create = useCreateEntity();

  const {
    register,
    handleSubmit,
    reset,
    formState: { errors },
  } = useForm<CreateForm>({
    resolver: zodResolver(createSchema),
    defaultValues: {
      kind: "bank",
      name: "",
      contact_name: "",
      email: "",
      phone: "",
      website: "",
      address: "",
      notes: "",
    },
  });

  function onOpenChange(next: boolean) {
    setOpen(next);
    if (!next) reset();
  }

  const onSubmit = handleSubmit((values) => {
    create.mutate(
      {
        kind: values.kind,
        name: values.name.trim(),
        contact_name: values.contact_name?.trim() || undefined,
        email: values.email?.trim() || undefined,
        phone: values.phone?.trim() || undefined,
        website: values.website?.trim() || undefined,
        address: values.address?.trim() || undefined,
        notes: values.notes?.trim() || undefined,
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
    <Dialog open={open} onOpenChange={onOpenChange}>
      <Button onClick={() => setOpen(true)}>
        <Plus className="h-4 w-4" />
        Add entity
      </Button>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>New entity</DialogTitle>
        </DialogHeader>
        <form onSubmit={onSubmit} className="space-y-4">
          <div className="grid gap-4 sm:grid-cols-2">
            <SelectField
              label="Kind"
              required
              error={errors.kind?.message}
              {...register("kind")}
            >
              {KINDS.map((k) => (
                <option key={k} value={k}>
                  {humanize(k)}
                </option>
              ))}
            </SelectField>
            <TextField
              label="Name"
              required
              placeholder="Acme Bank, N.A."
              error={errors.name?.message}
              {...register("name")}
            />
            <TextField
              label="Contact name"
              placeholder="Sam Ortiz"
              error={errors.contact_name?.message}
              {...register("contact_name")}
            />
            <TextField
              label="Email"
              type="email"
              placeholder="sam@acme.com"
              error={errors.email?.message}
              {...register("email")}
            />
            <TextField
              label="Phone"
              placeholder="(503) 555-0177"
              error={errors.phone?.message}
              {...register("phone")}
            />
            <TextField
              label="Website"
              placeholder="https://"
              error={errors.website?.message}
              {...register("website")}
            />
          </div>
          <TextField
            label="Address"
            error={errors.address?.message}
            {...register("address")}
          />
          <TextareaField
            label="Notes"
            rows={3}
            error={errors.notes?.message}
            {...register("notes")}
          />
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
              disabled={create.isPending}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={create.isPending}>
              {create.isPending ? "Saving…" : "Create entity"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
