"use client";

// Platform overview — the Acre HQ home for software staff. Aggregate metrics
// across every tenant on the platform, plus the tenant directory with a quick
// "provision a new tenant" flow. Staff-only (is_platform_staff + platform:admin).

import { useState } from "react";
import { useRouter } from "next/navigation";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import {
  Building2,
  CheckCircle2,
  DollarSign,
  Globe,
  Layers,
  Plus,
} from "lucide-react";

import { useAuth } from "@/lib/auth";
import { usePlatformMetrics, usePlatformTenants, useCreateTenant } from "@/lib/queries";
import type { TenantSummary } from "@/lib/api";
import { titleCase } from "@/lib/format";

import { Badge, statusTone } from "@/components/ui";
import { Button } from "@/components/ui/button";
import { Card, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { PageHeader, StatCard, EmptyState } from "@/components/ui/page";
import { DataTable, type ColumnDef } from "@/components/ui/data-table";
import { Field, TextField } from "@/components/ui/form-field";
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
  DialogTrigger,
} from "@/components/ui/dialog";

const PLANS = ["starter", "growth", "scale", "enterprise"] as const;

/** "New tenant" form. `slug` becomes the tenant's stable URL/header key. */
const createTenantSchema = z.object({
  name: z.string().trim().min(2, "Enter the company name."),
  slug: z
    .string()
    .trim()
    .min(2, "Give the tenant a slug (2+ characters).")
    .regex(/^[a-z0-9-]+$/, "Use lowercase letters, digits and hyphens."),
  plan: z.enum(PLANS),
});

type CreateTenantForm = z.infer<typeof createTenantSchema>;

const columns: ColumnDef<TenantSummary>[] = [
  {
    accessorKey: "name",
    header: "Name",
    cell: ({ row }) => {
      const t = row.original;
      return (
        <div className="min-w-0">
          <div className="truncate font-medium text-ink">{t.name}</div>
          <div className="truncate text-xs text-ink-3">{t.slug}</div>
        </div>
      );
    },
  },
  {
    accessorKey: "plan",
    header: "Plan",
    cell: ({ row }) => (
      <Badge tone="info">{titleCase(row.original.plan)}</Badge>
    ),
  },
  {
    accessorKey: "status",
    header: "Status",
    cell: ({ row }) => (
      <Badge tone={statusTone(row.original.status)}>
        {titleCase(row.original.status)}
      </Badge>
    ),
  },
  {
    accessorKey: "property_count",
    header: () => <div className="text-right">Properties</div>,
    cell: ({ row }) => (
      <div data-numeric className="text-right text-ink-2">
        {row.original.property_count}
      </div>
    ),
  },
  {
    accessorKey: "managed_revenue_label",
    header: () => <div className="text-right">Managed revenue</div>,
    cell: ({ row }) => (
      <div data-numeric className="text-right font-medium text-ink">
        {row.original.managed_revenue_label}
      </div>
    ),
  },
];

/** Acre HQ home: platform-wide metrics + the tenant directory. */
export default function PlatformPage() {
  const { can } = useAuth();
  const router = useRouter();
  const metrics = usePlatformMetrics();
  const tenants = usePlatformTenants();
  const canManage = can("platform:admin");

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Acre HQ"
        title="Platform overview"
        description="Every client company on the Acre platform, at a glance."
        actions={canManage ? <NewTenantDialog /> : undefined}
      />

      <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
        {metrics.isLoading ? (
          Array.from({ length: 4 }).map((_, i) => (
            <div key={i} className="skeleton h-[104px] rounded-xl" />
          ))
        ) : (
          <>
            <StatCard
              label="Tenants"
              value={metrics.data?.tenant_count ?? 0}
              sub="Client companies"
              icon={Globe}
            />
            <StatCard
              label="Active tenants"
              value={metrics.data?.active_tenants ?? 0}
              sub={`of ${metrics.data?.tenant_count ?? 0} total`}
              icon={CheckCircle2}
              tone="good"
            />
            <StatCard
              label="Total properties"
              value={metrics.data?.total_properties ?? 0}
              sub="Under management"
              icon={Building2}
            />
            <StatCard
              label="Managed revenue"
              value={metrics.data?.total_managed_revenue_label ?? "$0"}
              sub="Across all tenants"
              icon={DollarSign}
              tone="accent"
            />
          </>
        )}
      </div>

      <Card>
        <CardHeader>
          <div className="min-w-0">
            <CardTitle>Tenants</CardTitle>
            <CardDescription>
              Select a tenant to view its detail and rollups.
            </CardDescription>
          </div>
          {canManage && <NewTenantDialog />}
        </CardHeader>
        <div className="p-4">
          <DataTable<TenantSummary>
            columns={columns}
            data={tenants.data ?? []}
            isLoading={tenants.isLoading}
            searchPlaceholder="Search tenants…"
            onRowClick={(t) => router.push(`/console/platform/tenants/${t.id}`)}
            emptyState={
              <EmptyState
                className="border-0"
                icon={Layers}
                title="No tenants yet"
                description="Provision your first client company to start onboarding their portfolio."
                action={canManage ? <NewTenantDialog /> : undefined}
              />
            }
          />
        </div>
      </Card>
    </div>
  );
}

/** Dialog to provision a new tenant, then route to its detail page. */
function NewTenantDialog() {
  const [open, setOpen] = useState(false);
  const router = useRouter();
  const createTenant = useCreateTenant();

  const {
    register,
    handleSubmit,
    setValue,
    watch,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<CreateTenantForm>({
    resolver: zodResolver(createTenantSchema),
    defaultValues: { name: "", slug: "", plan: "starter" },
  });

  const plan = watch("plan");

  const close = () => {
    setOpen(false);
    reset({ name: "", slug: "", plan: "starter" });
  };

  const onSubmit = handleSubmit(async (values) => {
    const created = await createTenant.mutateAsync({
      slug: values.slug,
      name: values.name,
      plan: values.plan,
    });
    close();
    router.push(`/console/platform/tenants/${created.id}`);
  });

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => (next ? setOpen(true) : close())}
    >
      <DialogTrigger asChild>
        <Button>
          <Plus className="h-4 w-4" />
          New tenant
        </Button>
      </DialogTrigger>
      <DialogContent>
        <form onSubmit={onSubmit} className="space-y-5">
          <DialogHeader>
            <DialogTitle>New tenant</DialogTitle>
            <CardDescription>
              Provision a new client company on the platform.
            </CardDescription>
          </DialogHeader>

          <div className="space-y-4">
            <TextField
              label="Company name"
              placeholder="Cascade Living LLC"
              required
              error={errors.name?.message}
              {...register("name")}
            />
            <TextField
              label="Slug"
              placeholder="cascade"
              hint="Lowercase, used in URLs and the tenant header."
              required
              error={errors.slug?.message}
              {...register("slug")}
            />
            <Field label="Plan" required error={errors.plan?.message}>
              <Select
                value={plan}
                onValueChange={(v) =>
                  setValue("plan", v as CreateTenantForm["plan"], {
                    shouldValidate: true,
                  })
                }
              >
                <SelectTrigger className="h-10">
                  <SelectValue placeholder="Select a plan" />
                </SelectTrigger>
                <SelectContent>
                  {PLANS.map((p) => (
                    <SelectItem key={p} value={p}>
                      {titleCase(p)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </Field>
          </div>

          <DialogFooter>
            <Button type="button" variant="outline" onClick={close}>
              Cancel
            </Button>
            <Button type="submit" disabled={isSubmitting}>
              <Plus className="h-4 w-4" />
              {isSubmitting ? "Creating…" : "Create tenant"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
