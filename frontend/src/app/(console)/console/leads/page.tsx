"use client";

// Leads: prospective renters from the leasing inbox (mail to it creates or
// updates a lead) or entered by hand. Work one from first contact to a tour to
// an application. Reading needs application:read; working a lead needs
// application:write.

import { useMemo, useState } from "react";
import Link from "next/link";
import { motion } from "motion/react";
import {
  ArrowRight,
  CalendarPlus,
  Inbox,
  Plus,
  Search,
  UserRoundPlus,
} from "lucide-react";
import { useAuth } from "@/lib/auth";
import { useLeads, useUpdateLead } from "@/lib/queries";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge, type Tone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";
import type { Lead } from "@/lib/api";
import { CopyLink, humanize, NoAccess } from "../leases/_ui/shared";
import { ConvertDialog, NewLeadDialog, TourDialog } from "./Dialogs";

const STATUSES = ["new", "contacted", "toured", "applied", "closed"];

function leadTone(status: string): Tone {
  switch (status) {
    case "new":
      return "info";
    case "contacted":
      return "accent";
    case "toured":
      return "warn";
    case "applied":
      return "good";
    default:
      return "neutral";
  }
}

export default function LeadsPage() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const allowed = can("application:read");
  const write = can("application:write");
  const [status, setStatus] = useState("");
  const [q, setQ] = useState("");
  const { data, error, isLoading } = useLeads(status || undefined, {
    enabled: scoped && allowed,
  });
  const update = useUpdateLead();
  const [creating, setCreating] = useState(false);
  const [touring, setTouring] = useState<Lead | null>(null);
  const [converting, setConverting] = useState<Lead | null>(null);

  const rows = useMemo(() => {
    const needle = q.trim().toLowerCase();
    return (data?.leads ?? []).filter(
      (l) =>
        !needle ||
        l.name.toLowerCase().includes(needle) ||
        l.email.toLowerCase().includes(needle) ||
        (l.phone ?? "").includes(needle)
    );
  }, [data, q]);

  if (!allowed) return <NoAccess what="leads" perm="application:read" />;

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Leasing"
        title="Leads"
        description="People interested in renting, from first contact to a signed application."
        actions={
          write && (
            <Button onClick={() => setCreating(true)}>
              <Plus />
              New lead
            </Button>
          )
        }
      />

      {data?.inbox_address && (
        <Panel className="flex flex-wrap items-center gap-3 p-4">
          <span className="flex size-10 shrink-0 items-center justify-center rounded-xl border border-line bg-fill text-fg-2 [&_svg]:size-[18px]">
            <Inbox />
          </span>
          <div className="min-w-0 flex-1 text-[13px] text-fg-2">
            Mail sent to{" "}
            <code className="rounded bg-fill-2 px-1.5 py-0.5 font-mono text-fg">
              {data.inbox_address}
            </code>{" "}
            creates or updates a lead on its own.
          </div>
          <CopyLink url={data.inbox_address} label="Copy address" />
        </Panel>
      )}

      <Panel className="overflow-hidden">
        <div className="flex flex-col gap-3 border-b border-line p-3 sm:flex-row sm:items-center sm:justify-between">
          <div
            className="flex max-w-full gap-1 overflow-x-auto rounded-xl bg-fill p-1"
            role="tablist"
          >
            {["", ...STATUSES].map((s) => (
              <button
                key={s}
                role="tab"
                type="button"
                aria-selected={status === s}
                onClick={() => setStatus(s)}
                className={cn(
                  "shrink-0 rounded-lg px-3 py-1.5 text-[13px] font-medium transition",
                  status === s
                    ? "bg-surface text-fg shadow-sm"
                    : "text-fg-3 hover:text-fg"
                )}
              >
                {s ? humanize(s) : "All"}
              </button>
            ))}
          </div>
          <div className="relative sm:w-64">
            <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-fg-3" />
            <Input
              value={q}
              onChange={(e) => setQ(e.target.value)}
              placeholder="Search name, email or phone"
              aria-label="Search leads"
              className="pl-9"
            />
          </div>
        </div>

        {isLoading && (
          <div className="space-y-2 p-3">
            {Array.from({ length: 5 }, (_, i) => (
              <Skeleton key={i} className="h-14" />
            ))}
          </div>
        )}
        {error && (
          <p className="p-4 text-[13px] text-bad">
            Couldn&apos;t load leads: {error.message}
          </p>
        )}
        {data && rows.length === 0 && (
          <EmptyState
            icon={<UserRoundPlus />}
            title={
              q
                ? "Nothing matches"
                : status
                  ? "No leads in this status"
                  : "No leads yet"
            }
            description={
              q || status
                ? undefined
                : "Add one, or they arrive on their own from the leasing inbox."
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
              className="flex flex-wrap items-center gap-3 px-4 py-3"
            >
              <div className="min-w-0 flex-1 basis-60">
                <div className="flex items-center gap-2">
                  <span className="truncate text-[14px] font-medium text-fg">
                    {l.name}
                  </span>
                  <Badge tone={leadTone(l.status)}>{l.status}</Badge>
                </div>
                <div className="truncate text-xs text-fg-3">
                  {l.email}
                  {l.phone ? ` · ${l.phone}` : ""} · {humanize(l.source)}
                </div>
                {l.last_message && (
                  <div className="mt-0.5 line-clamp-1 text-xs text-fg-3">
                    “{l.last_message}”
                  </div>
                )}
              </div>
              {write && (
                <div className="flex flex-wrap items-center gap-2">
                  <select
                    aria-label={`Status of ${l.name}`}
                    value={l.status}
                    disabled={update.isPending}
                    onChange={(e) =>
                      update.mutate({
                        id: l.id,
                        body: { status: e.target.value },
                      })
                    }
                    className="rounded-lg border border-line bg-surface px-2 py-1.5 text-xs text-fg"
                  >
                    {STATUSES.map((s) => (
                      <option key={s} value={s}>
                        {humanize(s)}
                      </option>
                    ))}
                  </select>
                  <Button
                    size="sm"
                    variant="secondary"
                    onClick={() => setTouring(l)}
                  >
                    <CalendarPlus />
                    Tour
                  </Button>
                  {l.application_id ? (
                    <Button size="sm" variant="ghost" asChild>
                      <Link href="/console/applications">
                        Application
                        <ArrowRight />
                      </Link>
                    </Button>
                  ) : (
                    <Button size="sm" onClick={() => setConverting(l)}>
                      Convert
                    </Button>
                  )}
                </div>
              )}
            </motion.li>
          ))}
        </ul>
      </Panel>

      <NewLeadDialog open={creating} onOpenChange={setCreating} />
      <TourDialog lead={touring} onClose={() => setTouring(null)} />
      <ConvertDialog lead={converting} onClose={() => setConverting(null)} />
    </div>
  );
}
