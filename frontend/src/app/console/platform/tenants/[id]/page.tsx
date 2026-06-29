"use client";

// Platform tenant detail (Acre HQ, staff-only): a single client company's
// rollups + an inline "Manage" panel to change its status, plan, name and
// custom domain. All reads/writes go through TanStack Query hooks; mutations
// are gated by `platform:admin` and surface saving state + toasts.

import { useEffect, useMemo, useState } from "react";
import { useParams } from "next/navigation";
import {
  Banknote,
  Building2,
  CreditCard,
  Globe,
  ShieldCheck,
  Users,
} from "lucide-react";

import { useAuth } from "@/lib/auth";
import { usePlatformTenant, useUpdateTenant } from "@/lib/queries";
import type { TenantDetail, UpdateTenantInput } from "@/lib/api";
import { currencyFromCents, formatDate, titleCase } from "@/lib/format";

import { Badge, statusTone } from "@/components/ui";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { PageHeader, StatCard, EmptyState } from "@/components/ui/page";
import { Breadcrumbs } from "@/components/ui/breadcrumbs";
import { Field, TextField } from "@/components/ui/form-field";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

const STATUSES = ["active", "suspended", "trial"] as const;
const PLANS = ["starter", "growth", "enterprise"] as const;

/** Platform-staff detail view for a single tenant (client company). */
export default function TenantDetailPage() {
  const params = useParams<{ id: string }>();
  const { can, user: me } = useAuth();
  const tenant = usePlatformTenant(params.id);

  if (!me?.is_platform_staff) {
    return (
      <Card>
        <CardContent>
          <p className="text-ink-2">
            This is the platform (Acre HQ) admin — staff only.
          </p>
        </CardContent>
      </Card>
    );
  }

  if (tenant.error) {
    return (
      <div className="space-y-6">
        <Breadcrumbs
          items={[
            { label: "Platform", href: "/console/platform" },
            { label: "Tenants", href: "/console/platform" },
            { label: "Not found" },
          ]}
        />
        <EmptyState
          icon={Building2}
          title="Couldn't load tenant"
          description={tenant.error.message}
        />
      </div>
    );
  }

  if (tenant.isLoading || !tenant.data) {
    return (
      <div className="space-y-6">
        <div className="skeleton h-4 w-48 rounded" />
        <div className="skeleton h-12 w-72 rounded-lg" />
        <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
          {Array.from({ length: 4 }).map((_, i) => (
            <div key={i} className="skeleton h-[104px] rounded-xl" />
          ))}
        </div>
        <div className="skeleton h-72 rounded-xl" />
      </div>
    );
  }

  const t = tenant.data;
  const canManage = can("platform:admin");

  return (
    <div className="space-y-6">
      <Breadcrumbs
        items={[
          { label: "Platform", href: "/console/platform" },
          { label: "Tenants", href: "/console/platform" },
          { label: t.name },
        ]}
      />

      <PageHeader
        eyebrow="Acre HQ"
        title={
          <span className="flex items-center gap-3">
            {t.name}
            <Badge tone={statusTone(t.status)}>{titleCase(t.status)}</Badge>
          </span>
        }
        description={
          <span className="font-mono text-ink-3">{t.slug}</span>
        }
      />

      {/* Rollups */}
      <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
        <StatCard
          label="Properties"
          value={t.property_count}
          sub="Under management"
          icon={Building2}
        />
        <StatCard
          label="Members"
          value={t.member_count}
          sub="With workspace access"
          icon={Users}
        />
        <StatCard
          label="Managed revenue"
          value={t.managed_revenue_label || currencyFromCents(t.revenue_cents)}
          sub="Gross scheduled rent"
          icon={Banknote}
          tone="good"
        />
        <StatCard
          label="Plan"
          value={titleCase(t.plan)}
          sub={`Since ${formatDate(t.created_at)}`}
          icon={CreditCard}
          tone="accent"
        />
      </div>

      <div className="grid gap-6 lg:grid-cols-[1.4fr_1fr]">
        <ManageCard tenant={t} canManage={canManage} />
        <OverviewCard tenant={t} />
      </div>
    </div>
  );
}

/** Read-only summary of the tenant's identity + footprint. */
function OverviewCard({ tenant: t }: { tenant: TenantDetail }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Overview</CardTitle>
      </CardHeader>
      <CardContent>
        <dl className="space-y-3 text-sm">
          <Row label="Slug" value={t.slug} mono />
          <Row
            label="Custom domain"
            value={t.custom_domain ?? undefined}
            mono
            icon={t.custom_domain ? Globe : undefined}
          />
          <Row label="Plan" value={titleCase(t.plan)} />
          <Row
            label="Status"
            value={<Badge tone={statusTone(t.status)}>{titleCase(t.status)}</Badge>}
          />
          <Row label="Properties" value={String(t.property_count)} />
          <Row label="Members" value={String(t.member_count)} />
          <Row
            label="Managed revenue"
            value={t.managed_revenue_label || currencyFromCents(t.revenue_cents)}
            mono
          />
          <Row label="Created" value={formatDate(t.created_at)} />
        </dl>
      </CardContent>
    </Card>
  );
}

/** Inline management panel: status / plan / name / custom domain. */
function ManageCard({
  tenant: t,
  canManage,
}: {
  tenant: TenantDetail;
  canManage: boolean;
}) {
  const update = useUpdateTenant();

  const [status, setStatus] = useState(t.status);
  const [plan, setPlan] = useState(t.plan);
  const [name, setName] = useState(t.name);
  const [customDomain, setCustomDomain] = useState(t.custom_domain ?? "");

  // Re-sync local form state when the tenant record refreshes (e.g. after save).
  useEffect(() => {
    setStatus(t.status);
    setPlan(t.plan);
    setName(t.name);
    setCustomDomain(t.custom_domain ?? "");
  }, [t.status, t.plan, t.name, t.custom_domain]);

  const trimmedName = name.trim();
  const trimmedDomain = customDomain.trim();
  const currentDomain = t.custom_domain ?? "";

  const dirty = useMemo(
    () =>
      status !== t.status ||
      plan !== t.plan ||
      trimmedName !== t.name ||
      trimmedDomain !== currentDomain,
    [status, plan, trimmedName, trimmedDomain, t, currentDomain]
  );

  function onSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!canManage || !dirty || !trimmedName) return;

    const body: UpdateTenantInput = {};
    if (status !== t.status) body.status = status;
    if (plan !== t.plan) body.plan = plan;
    if (trimmedName !== t.name) body.name = trimmedName;
    if (trimmedDomain !== currentDomain) body.custom_domain = trimmedDomain;

    update.mutate({ id: t.id, body });
  }

  function onReset() {
    setStatus(t.status);
    setPlan(t.plan);
    setName(t.name);
    setCustomDomain(t.custom_domain ?? "");
  }

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle>Manage</CardTitle>
          <CardDescription className="mt-0.5">
            Change this company&apos;s status, plan, and identity.
          </CardDescription>
        </div>
      </CardHeader>
      <CardContent>
        {!canManage ? (
          <div className="flex items-center gap-3 rounded-lg border border-line bg-surface-2 px-4 py-3 text-sm text-ink-2">
            <ShieldCheck className="h-4 w-4 shrink-0 text-ink-3" />
            You need the <span className="font-mono text-ink">platform:admin</span>{" "}
            permission to change tenant settings.
          </div>
        ) : (
          <form onSubmit={onSubmit} className="space-y-5">
            <div className="grid gap-4 sm:grid-cols-2">
              <Field label="Status">
                <Select
                  value={status}
                  onValueChange={setStatus}
                  disabled={update.isPending}
                >
                  <SelectTrigger className="h-10">
                    <SelectValue placeholder="Select status" />
                  </SelectTrigger>
                  <SelectContent>
                    {STATUSES.map((s) => (
                      <SelectItem key={s} value={s}>
                        {titleCase(s)}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>

              <Field label="Plan">
                <Select
                  value={plan}
                  onValueChange={setPlan}
                  disabled={update.isPending}
                >
                  <SelectTrigger className="h-10">
                    <SelectValue placeholder="Select plan" />
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

            <TextField
              label="Company name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Acme Property Group"
              required
              disabled={update.isPending}
              error={!trimmedName ? "Name is required." : undefined}
            />

            <TextField
              label="Custom domain"
              value={customDomain}
              onChange={(e) => setCustomDomain(e.target.value)}
              placeholder="app.acme.com"
              hint="The white-label hostname this tenant is served on. Leave blank for none."
              disabled={update.isPending}
            />

            <div className="flex items-center justify-end gap-2 pt-1">
              <Button
                type="button"
                variant="outline"
                onClick={onReset}
                disabled={update.isPending || !dirty}
              >
                Reset
              </Button>
              <Button type="submit" disabled={update.isPending || !dirty || !trimmedName}>
                {update.isPending ? "Saving…" : "Save changes"}
              </Button>
            </div>
          </form>
        )}
      </CardContent>
    </Card>
  );
}

/** A definition-list row; renders an em-dash for empty values. */
function Row({
  label,
  value,
  mono,
  icon: Icon,
}: {
  label: string;
  value?: React.ReactNode;
  mono?: boolean;
  icon?: typeof Globe;
}) {
  const empty = value == null || value === "";
  return (
    <div className="flex items-center justify-between gap-4 border-b border-line pb-3 last:border-0 last:pb-0">
      <dt className="text-ink-3">{label}</dt>
      <dd
        className={
          empty
            ? "text-ink-3"
            : mono
              ? "flex items-center gap-1.5 font-mono font-medium text-ink"
              : "flex items-center gap-1.5 font-medium text-ink"
        }
      >
        {!empty && Icon && <Icon className="h-3.5 w-3.5 text-ink-3" />}
        {empty ? "—" : value}
      </dd>
    </div>
  );
}
