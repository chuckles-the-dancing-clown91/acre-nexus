"use client";

// One lease, in full: who and where, the money (rent, charges, vehicles and the
// payment ledger), the document and its signatures, renewals, inspections, the
// deposit, and stored files. Reads need lease:read; changes need lease:manage.

import { Suspense } from "react";
import Link from "next/link";
import { useParams, useRouter, useSearchParams } from "next/navigation";
import { useQuery } from "@tanstack/react-query";
import { motion } from "motion/react";
import { ArrowLeft, Building2, FileText, Receipt } from "lucide-react";
import { api, ApiError } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { usd } from "@/lib/format";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge, statusTone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DataTable } from "@/components/ui/data-table";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";
import type { LeaseDetail } from "@/lib/types";
import { NoAccess, paymentTone } from "../_ui/shared";
import { Charges } from "./Charges";
import { Deposit } from "./Deposit";
import { Esign } from "./Esign";
import { Files } from "./Files";
import { Inspections } from "./Inspections";
import { LeaseDocument, useLeaseDoc } from "./LeaseDocument";
import { Renewals } from "./Renewals";
import { Vehicles } from "./Vehicles";

const TABS = [
  { key: "overview", label: "Rent and payments" },
  { key: "signing", label: "Document and signing" },
  { key: "renewals", label: "Renewals" },
  { key: "inspections", label: "Inspections" },
  { key: "deposit", label: "Deposit" },
  { key: "files", label: "Files" },
] as const;
type TabKey = (typeof TABS)[number]["key"];

const rise = (i: number) => ({
  initial: { opacity: 0, y: 10 },
  animate: { opacity: 1, y: 0 },
  transition: {
    duration: 0.45,
    delay: i * 0.04,
    ease: [0.22, 1, 0.36, 1] as const,
  },
});

export default function LeasePage() {
  return (
    <Suspense fallback={<Skeleton className="h-64" />}>
      <LeaseView />
    </Suspense>
  );
}

function LeaseView() {
  const { id } = useParams<{ id: string }>();
  const { can, user } = useAuth();
  const scoped = useHasTenantScope();
  const params = useSearchParams();
  const router = useRouter();
  const allowed = can("lease:read");
  const manage = can("lease:manage");
  const tab: TabKey =
    TABS.find((t) => t.key === params.get("tab"))?.key ?? "overview";

  const lease = useQuery({
    queryKey: ["leases", id],
    queryFn: () => api.lease(id),
    enabled: scoped && allowed,
    retry: (n, e) => !(e instanceof ApiError && e.status < 500) && n < 2,
  });
  const charges = useQuery({
    queryKey: ["leases", id, "charges"],
    queryFn: () => api.leaseCharges(id),
    enabled: scoped && allowed,
  });
  const property = useQuery({
    queryKey: ["properties", lease.data?.property_id ?? ""],
    queryFn: () => api.property(lease.data?.property_id ?? ""),
    enabled: !!lease.data,
  });
  const doc = useLeaseDoc(id, scoped && allowed);

  if (!allowed) return <NoAccess what="leases" perm="lease:read" />;

  if (lease.error) {
    const missing =
      lease.error instanceof ApiError && lease.error.status === 404;
    return (
      <Panel className="mx-auto mt-10 max-w-lg">
        <EmptyState
          icon={<FileText />}
          title={
            missing
              ? "This lease isn't in your view"
              : "Couldn't load the lease"
          }
          description={
            missing
              ? "It may belong to another company, or to a property you aren't assigned to."
              : lease.error.message
          }
          action={
            <Button variant="secondary" asChild>
              <Link href="/console/leases">
                <ArrowLeft />
                All leases
              </Link>
            </Button>
          }
        />
      </Panel>
    );
  }

  const l = lease.data;

  return (
    <div className="space-y-6">
      <Link
        href="/console/leases"
        className="inline-flex items-center gap-1.5 text-[13px] text-fg-3 transition hover:text-fg"
      >
        <ArrowLeft className="size-4" />
        Leases
      </Link>

      <motion.header {...rise(0)} className="space-y-2">
        {l ? (
          <>
            <div className="flex flex-wrap items-center gap-2">
              <h1 className="text-[28px] leading-tight font-semibold text-fg sm:text-[32px]">
                {l.tenant_name}
              </h1>
              <Badge tone={statusTone(l.status)}>{l.status}</Badge>
              <Badge tone={paymentTone(l.payment_status)}>
                {l.payment_status}
              </Badge>
              {l.application_id && <Badge tone="info">from application</Badge>}
              {l.has_pet && <Badge tone="warn">pet</Badge>}
              {l.is_military && <Badge tone="info">military</Badge>}
            </div>
            <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-[14px] text-fg-2">
              <span>{l.tenant_email ?? "No email"}</span>
              {l.tenant_phone && <span>{l.tenant_phone}</span>}
              <span>
                {l.start_date}
                {l.end_date ? ` to ${l.end_date}` : ", month to month"}
              </span>
              <Link
                href={`/console/properties/${l.property_id}`}
                className="inline-flex items-center gap-1 text-accent hover:underline"
              >
                <Building2 className="size-3.5" />
                {property.data?.name ?? "Property"}
              </Link>
            </div>
            {(l.pet_details || l.notes) && (
              <p className="max-w-2xl text-[13px] text-fg-3">
                {[l.pet_details && `Pet: ${l.pet_details}`, l.notes]
                  .filter(Boolean)
                  .join(" · ")}
              </p>
            )}
          </>
        ) : (
          <>
            <Skeleton className="h-9 w-72" />
            <Skeleton className="h-5 w-96" />
          </>
        )}
      </motion.header>

      <section className="grid grid-cols-2 gap-3 xl:grid-cols-4">
        <motion.div {...rise(1)}>
          <Stat
            label="Rent"
            value={l ? l.rent_label : "—"}
            hint={
              charges.data
                ? `${charges.data.monthly_total_label} with charges`
                : "per month"
            }
          />
        </motion.div>
        <motion.div {...rise(2)}>
          <Stat
            label="Balance"
            value={l ? usd(Math.max(0, l.balance_cents)) : "—"}
            hint={l && l.balance_cents > 0 ? "owed now" : "nothing owed"}
            tone={l && l.balance_cents > 0 ? "bad" : undefined}
          />
        </motion.div>
        <motion.div {...rise(3)}>
          <Stat
            label="Deposit"
            value={l?.deposit_label ?? "None"}
            hint={l?.deposit_label ? "see the deposit tab" : undefined}
          />
        </motion.div>
        <motion.div {...rise(4)}>
          <Stat
            label="Term ends"
            value={l ? (l.end_date ?? "Month to month") : "—"}
            hint={l ? `started ${l.start_date}` : undefined}
            small
          />
        </motion.div>
      </section>

      <nav
        className="-mx-1 flex gap-1 overflow-x-auto border-b border-line px-1"
        aria-label="Lease sections"
      >
        {TABS.map((t) => (
          <button
            key={t.key}
            type="button"
            aria-current={tab === t.key ? "page" : undefined}
            onClick={() =>
              router.replace(t.key === "overview" ? "?" : `?tab=${t.key}`, {
                scroll: false,
              })
            }
            className={cn(
              "-mb-px shrink-0 border-b-2 px-3 py-2.5 text-[13px] font-medium whitespace-nowrap transition",
              tab === t.key
                ? "border-accent text-fg"
                : "border-transparent text-fg-3 hover:text-fg"
            )}
          >
            {t.label}
          </button>
        ))}
      </nav>

      {!l && <Skeleton className="h-64 rounded-2xl" />}

      {l && tab === "overview" && (
        <div className="grid gap-4 xl:grid-cols-[minmax(0,1.6fr)_minmax(0,1fr)]">
          <div className="space-y-4">
            <Charges leaseId={id} manage={manage} />
            <Payments lease={l} />
          </div>
          <Vehicles leaseId={id} manage={can("vehicle:manage")} />
        </div>
      )}

      {l && tab === "signing" && (
        <div className="space-y-4">
          <LeaseDocument leaseId={id} manage={manage} />
          {!doc.isLoading && (
            <Esign
              leaseId={id}
              manage={manage}
              hasDocument={!!doc.data}
              documentSigned={doc.data?.status === "signed"}
              defaultSigners={[
                {
                  role: "resident",
                  name: l.tenant_name,
                  email: l.tenant_email ?? "",
                  phone: l.tenant_phone ?? undefined,
                },
                {
                  role: "landlord",
                  name: user?.name ?? "",
                  email: user?.email ?? "",
                },
              ]}
            />
          )}
        </div>
      )}

      {l && tab === "renewals" && <Renewals lease={l} manage={manage} />}
      {l && tab === "inspections" && (
        <Inspections leaseId={id} manage={manage} />
      )}
      {l && tab === "deposit" && <Deposit leaseId={id} manage={manage} />}
      {l && tab === "files" && (
        <Files
          ownerType="lease"
          ownerId={id}
          title="Files"
          description="Signed PDFs, addenda, deposit statements and move-in photos."
        />
      )}
    </div>
  );
}

function Payments({ lease }: { lease: LeaseDetail }) {
  return (
    <Panel>
      <PanelHeader
        title="Payment ledger"
        description={`${lease.payments.length} ${lease.payments.length === 1 ? "payment" : "payments"} on record`}
      />
      <div className="p-2 pt-3">
        {lease.payments.length === 0 ? (
          <EmptyState
            icon={<Receipt />}
            title="No payments yet"
            className="py-8"
          />
        ) : (
          <DataTable
            columns={[
              "Due",
              "Paid",
              "Method",
              { label: "Amount", num: true },
              "Status",
            ]}
            rows={lease.payments.map((p) => [
              p.due_date,
              p.paid_date ?? "—",
              p.method ?? "—",
              p.amount_label,
              <Badge key="s" tone={statusTone(p.status)}>
                {p.status}
              </Badge>,
            ])}
          />
        )}
      </div>
    </Panel>
  );
}

function Stat({
  label,
  value,
  hint,
  tone,
  small,
}: {
  label: string;
  value: string;
  hint?: string;
  tone?: "bad";
  small?: boolean;
}) {
  return (
    <Panel className="flex h-full flex-col p-5">
      <div className="eyebrow">{label}</div>
      <div
        className={cn(
          small
            ? "mt-2 truncate text-[17px] font-semibold"
            : "figure mt-2 text-[28px] leading-none font-semibold",
          tone === "bad" ? "text-bad" : "text-fg"
        )}
      >
        {value}
      </div>
      {hint && <div className="mt-2 text-xs text-fg-3">{hint}</div>}
    </Panel>
  );
}
