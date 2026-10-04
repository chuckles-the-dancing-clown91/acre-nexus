"use client";

// Foundation: every lease in an entity run as a foundation, with where its
// income certification stands and what the housing authority still owes.

import Link from "next/link";
import { useQuery } from "@tanstack/react-query";
import { HandHeart } from "lucide-react";
import { CERT_WORDS, certTone, family } from "@/lib/family";
import { useAuth } from "@/lib/auth";
import { usd } from "@/lib/format";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge } from "@/components/ui/badge";
import { Stat } from "@/components/ui/data-table";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";

export default function FoundationPage() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const q = useQuery({
    queryKey: ["foundation-compliance"],
    queryFn: family.compliance,
    enabled: scoped && can("lease:read"),
  });
  const rows = q.data ?? [];
  const due = rows.filter((r) => r.certification !== "ok").length;
  const owed = rows.reduce((s, r) => s + r.hap_owed_cents, 0);
  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Leasing"
        title="Foundation"
        description="Leases in entities run as a foundation: income certifications and housing vouchers."
      />
      {q.isLoading && <Skeleton className="h-40 rounded-2xl" />}
      {q.data && rows.length === 0 && (
        <EmptyState
          icon={<HandHeart />}
          title="No foundation leases"
          description="Turn on foundation mode for an LLC on its page, and its active leases show up here."
        />
      )}
      {rows.length > 0 && (
        <>
          <div className="grid grid-cols-2 gap-3 lg:grid-cols-3">
            <Stat label="Leases" value={rows.length} />
            <Stat
              label="Need a certification"
              value={due}
              tone={due ? "warn" : undefined}
            />
            <Stat label="Housing authority owes" value={usd(owed)} />
          </div>
          <Panel className="overflow-hidden">
            <ul className="divide-y divide-line">
              {rows.map((r) => (
                <li key={r.lease_id}>
                  <Link
                    href={`/console/leases/${r.lease_id}?tab=assistance`}
                    className="flex flex-wrap items-center gap-3 px-5 py-3 transition hover:bg-fill"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="text-[14px] font-semibold text-fg">
                        {r.tenant_name}
                      </div>
                      <div className="text-[12px] text-fg-3">
                        {r.property_name} · {r.entity_name}
                        {r.authority &&
                          ` · ${r.authority} pays ${usd(r.hap_cents ?? 0)}/mo`}
                      </div>
                    </div>
                    {r.hap_owed_cents > 0 && (
                      <Badge tone="warn">{usd(r.hap_owed_cents)} HAP due</Badge>
                    )}
                    <Badge tone={certTone(r.certification)}>
                      {CERT_WORDS[r.certification]}
                      {r.expires_on && r.certification !== "missing"
                        ? ` · ${r.expires_on}`
                        : ""}
                    </Badge>
                  </Link>
                </li>
              ))}
            </ul>
          </Panel>
        </>
      )}
    </div>
  );
}
