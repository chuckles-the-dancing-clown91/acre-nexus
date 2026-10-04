"use client";

// Tenant history: every resident the company has leased to, past and present,
// with each of their tenancies. Gated by lease:read.

import { useMemo, useState } from "react";
import Link from "next/link";
import { useQuery } from "@tanstack/react-query";
import { motion } from "motion/react";
import { ChevronRight, History, Search } from "lucide-react";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge, statusTone } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";
import { NoAccess } from "../leases/_ui/shared";

export default function TenantHistoryPage() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const allowed = can("lease:read");
  const history = useQuery({
    queryKey: ["tenant-history"],
    queryFn: () => api.tenantHistory(),
    enabled: scoped && allowed,
  });
  const [q, setQ] = useState("");
  const [who, setWho] = useState<"all" | "current" | "former">("all");

  const all = useMemo(() => history.data ?? [], [history.data]);
  const rows = useMemo(() => {
    const needle = q.trim().toLowerCase();
    return all
      .filter((r) => who === "all" || (who === "current") === r.current)
      .filter(
        (r) =>
          !needle ||
          r.tenant_name.toLowerCase().includes(needle) ||
          (r.tenant_email ?? "").toLowerCase().includes(needle)
      );
  }, [all, q, who]);
  const current = all.filter((r) => r.current).length;

  if (!allowed) return <NoAccess what="tenant history" perm="lease:read" />;

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Leasing"
        title="Tenant history"
        description="Every resident you've leased to, past and present."
      />

      {history.data && all.length > 0 && (
        <div className="flex flex-wrap items-center gap-2">
          {(
            [
              ["all", `All · ${all.length}`],
              ["current", `Current · ${current}`],
              ["former", `Former · ${all.length - current}`],
            ] as const
          ).map(([k, label]) => (
            <button
              key={k}
              type="button"
              aria-pressed={who === k}
              onClick={() => setWho(k)}
              className={cn(
                "rounded-full border px-3 py-1 text-[13px] transition",
                who === k
                  ? "border-accent bg-accent/10 text-accent"
                  : "border-line text-fg-3 hover:text-fg"
              )}
            >
              {label}
            </button>
          ))}
          <div className="relative w-full sm:ml-auto sm:w-64">
            <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-fg-3" />
            <Input
              value={q}
              onChange={(e) => setQ(e.target.value)}
              placeholder="Search name or email"
              aria-label="Search tenants"
              className="h-10 pl-9"
            />
          </div>
        </div>
      )}

      {history.error && (
        <Panel className="border-bad/30 p-4 text-[13px] text-bad">
          Couldn&apos;t load tenant history: {history.error.message}
        </Panel>
      )}

      {history.isLoading && (
        <div className="space-y-3">
          {Array.from({ length: 4 }, (_, i) => (
            <Skeleton key={i} className="h-28 rounded-2xl" />
          ))}
        </div>
      )}

      {history.data && rows.length === 0 && (
        <Panel>
          <EmptyState
            icon={q ? <Search /> : <History />}
            title={all.length === 0 ? "No tenants yet" : "Nothing matches"}
            description={
              all.length === 0
                ? "Residents show up here once they have a lease."
                : undefined
            }
          />
        </Panel>
      )}

      <div className="space-y-3">
        {rows.map((r, i) => (
          <motion.div
            key={`${r.tenant_name}-${r.tenant_email ?? ""}`}
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: Math.min(i, 12) * 0.03, duration: 0.35 }}
          >
            <Panel className="overflow-hidden">
              <div className="flex flex-wrap items-center gap-3 border-b border-line px-4 py-3">
                <span className="flex size-9 shrink-0 items-center justify-center rounded-full bg-accent/15 text-[12px] font-semibold text-accent">
                  {r.tenant_name
                    .split(" ")
                    .map((w) => w[0])
                    .slice(0, 2)
                    .join("")}
                </span>
                <div className="min-w-0 flex-1">
                  {r.tenant_email ? (
                    <Link
                      href={`/console/residents?email=${encodeURIComponent(r.tenant_email)}`}
                      className="block truncate text-[15px] font-semibold text-fg hover:text-accent"
                    >
                      {r.tenant_name}
                    </Link>
                  ) : (
                    <div className="truncate text-[15px] font-semibold text-fg">
                      {r.tenant_name}
                    </div>
                  )}
                  <div className="truncate text-xs text-fg-3">
                    {r.tenant_email ?? "No email"}
                    {r.tenant_phone ? ` · ${r.tenant_phone}` : ""}
                  </div>
                </div>
                <Badge tone={r.current ? "good" : "neutral"}>
                  {r.current ? "current resident" : "former"}
                </Badge>
                <span className="text-xs text-fg-3">
                  {r.lease_count}{" "}
                  {r.lease_count === 1 ? "tenancy" : "tenancies"}
                </span>
              </div>
              <ul className="divide-y divide-line">
                {r.tenancies.map((t) => (
                  <li key={t.lease_id}>
                    <Link
                      href={`/console/leases/${t.lease_id}`}
                      className="flex flex-wrap items-center gap-3 px-4 py-2.5 text-[13px] transition hover:bg-fill-2"
                    >
                      <span className="min-w-[160px] flex-1 font-medium text-fg">
                        {t.property_name ?? "Property"}
                      </span>
                      <span className="text-fg-3">
                        {t.start_date} to {t.end_date ?? "now"}
                      </span>
                      <span className="figure text-fg-2">
                        {t.rent_label}/mo
                      </span>
                      {t.from_application && <Badge tone="info">applied</Badge>}
                      <Badge tone={statusTone(t.status)}>{t.status}</Badge>
                      {t.balance_cents > 0 && (
                        <Badge tone="bad">owes {t.balance_label}</Badge>
                      )}
                      <ChevronRight className="size-4 text-fg-4" />
                    </Link>
                  </li>
                ))}
              </ul>
            </Panel>
          </motion.div>
        ))}
      </div>
    </div>
  );
}
