"use client";

// Leases: every tenancy the viewer can see, with its rent and how the payments
// stand. Filter by status, search by tenant, open one for the full record.

import { useMemo, useState } from "react";
import Link from "next/link";
import { useQuery } from "@tanstack/react-query";
import { motion } from "motion/react";
import {
  CircleDollarSign,
  FileText,
  History,
  Search,
  TriangleAlert,
  Users,
} from "lucide-react";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { usd } from "@/lib/format";
import { useProperties } from "@/lib/queries";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge, statusTone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";
import { IconStat as Stat, NoAccess, paymentTone } from "./_ui/shared";

const STATUSES = [
  ["", "All"],
  ["active", "Active"],
  ["upcoming", "Upcoming"],
  ["notice", "Notice"],
  ["expired", "Expired"],
  ["ended", "Ended"],
] as const;

export default function LeasesPage() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const allowed = can("lease:read");
  const [status, setStatus] = useState("");
  const [q, setQ] = useState("");
  const leases = useQuery({
    queryKey: ["leases", { status }],
    queryFn: () => api.leases(status ? { status } : {}),
    enabled: scoped && allowed,
  });
  const properties = useProperties({ enabled: scoped && allowed });

  const names = useMemo(
    () => new Map((properties.data ?? []).map((p) => [p.id, p.name])),
    [properties.data]
  );
  const all = useMemo(() => leases.data ?? [], [leases.data]);
  const rows = useMemo(() => {
    const needle = q.trim().toLowerCase();
    if (!needle) return all;
    return all.filter(
      (l) =>
        l.tenant_name.toLowerCase().includes(needle) ||
        (l.tenant_email ?? "").toLowerCase().includes(needle) ||
        (names.get(l.property_id) ?? "").toLowerCase().includes(needle)
    );
  }, [all, q, names]);
  const totals = useMemo(
    () => ({
      count: all.length,
      rent: all.reduce((n, l) => n + l.rent_cents, 0),
      late: all.filter((l) => l.payment_status === "late").length,
      owed: all.reduce((n, l) => n + Math.max(0, l.balance_cents), 0),
    }),
    [all]
  );

  if (!allowed) return <NoAccess what="leases" perm="lease:read" />;

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Leasing"
        title="Leases"
        description="Every tenancy and how its payments stand."
        actions={
          <Button variant="secondary" asChild>
            <Link href="/console/tenant-history">
              <History />
              Tenant history
            </Link>
          </Button>
        }
      />

      <section className="grid grid-cols-2 gap-3 xl:grid-cols-4">
        <Stat
          icon={<FileText />}
          label={status ? `${status} leases` : "Leases"}
          value={leases.data ? String(totals.count) : "—"}
        />
        <Stat
          icon={<CircleDollarSign />}
          label="Monthly rent"
          value={leases.data ? usd(totals.rent) : "—"}
        />
        <Stat
          icon={<TriangleAlert />}
          label="Paying late"
          value={leases.data ? String(totals.late) : "—"}
          tone={totals.late ? "bad" : undefined}
        />
        <Stat
          icon={<Users />}
          label="Balance owed"
          value={leases.data ? usd(totals.owed) : "—"}
          tone={totals.owed ? "warn" : undefined}
        />
      </section>

      <Panel className="overflow-hidden">
        <div className="flex flex-col gap-3 border-b border-line p-3 sm:flex-row sm:items-center sm:justify-between">
          <div
            className="flex max-w-full gap-1 overflow-x-auto rounded-xl bg-fill p-1"
            role="tablist"
          >
            {STATUSES.map(([k, label]) => (
              <button
                key={k}
                role="tab"
                type="button"
                aria-selected={status === k}
                onClick={() => setStatus(k)}
                className={cn(
                  "shrink-0 rounded-lg px-3 py-1.5 text-[13px] font-medium transition",
                  status === k
                    ? "bg-surface text-fg shadow-sm"
                    : "text-fg-3 hover:text-fg"
                )}
              >
                {label}
              </button>
            ))}
          </div>
          <div className="relative sm:w-64">
            <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-fg-3" />
            <Input
              value={q}
              onChange={(e) => setQ(e.target.value)}
              placeholder="Search tenants or properties"
              aria-label="Search leases"
              className="pl-9"
            />
          </div>
        </div>

        {leases.isLoading && (
          <div className="space-y-2 p-3">
            {Array.from({ length: 5 }, (_, i) => (
              <Skeleton key={i} className="h-14" />
            ))}
          </div>
        )}
        {leases.error && (
          <p className="p-4 text-[13px] text-bad">
            Couldn&apos;t load leases: {leases.error.message}
          </p>
        )}
        {leases.data && rows.length === 0 && (
          <EmptyState
            icon={q ? <Search /> : <FileText />}
            title={q ? "Nothing matches" : "No leases here"}
            description={
              q
                ? `No lease matches “${q}”.`
                : "Approved applications become leases here."
            }
          />
        )}
        <ul className="divide-y divide-line">
          {rows.map((l, i) => (
            <motion.li
              key={l.id}
              initial={{ opacity: 0, y: 6 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: Math.min(i, 15) * 0.02, duration: 0.3 }}
            >
              <Link
                href={`/console/leases/${l.id}`}
                className="flex items-center gap-3 px-4 py-3 transition hover:bg-fill-2"
              >
                <span className="flex size-8 shrink-0 items-center justify-center rounded-full bg-accent/15 text-[12px] font-semibold text-accent">
                  {initials(l.tenant_name)}
                </span>
                <div className="min-w-0 flex-1">
                  <div className="truncate text-[14px] font-medium text-fg">
                    {l.tenant_name}
                  </div>
                  <div className="truncate text-xs text-fg-3">
                    {names.get(l.property_id) ?? "Property"}
                    {l.tenant_email ? ` · ${l.tenant_email}` : ""} ·{" "}
                    {l.start_date}
                    {l.end_date ? ` to ${l.end_date}` : ", month to month"}
                  </div>
                </div>
                {l.balance_cents > 0 && (
                  <span className="figure hidden text-xs text-bad sm:block">
                    owes {usd(l.balance_cents)}
                  </span>
                )}
                <span className="figure hidden w-24 text-right text-[13px] text-fg-2 md:block">
                  {l.rent_label}
                </span>
                <Badge tone={statusTone(l.status)}>{l.status}</Badge>
                <Badge
                  tone={paymentTone(l.payment_status)}
                  className="hidden sm:inline-flex"
                >
                  {l.payment_status}
                </Badge>
              </Link>
            </motion.li>
          ))}
        </ul>
      </Panel>
    </div>
  );
}

function initials(name: string) {
  return name
    .split(" ")
    .map((w) => w[0])
    .slice(0, 2)
    .join("");
}
