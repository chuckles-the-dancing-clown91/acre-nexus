"use client";

// Applications: every intake door (the website, the renter portal, the back
// office) lands here and moves through one pipeline, from screening to
// approved to leased. Move an application along, read its screening report,
// reuse it for another property, or turn an approved one into a lease.

import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { motion } from "motion/react";
import {
  ChevronDown,
  ClipboardList,
  Copy,
  FilePlus2,
  Plus,
  Search,
} from "lucide-react";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { useApplications, useProperties, useSettings } from "@/lib/queries";
import type { Application } from "@/lib/types";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge, statusTone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";
import { NoAccess, useRun } from "../leases/_ui/shared";
import { Pipeline, Screening } from "./Details";
import { ConvertDialog, IntakeDialog } from "./Forms";

type Open = { id: string; part: "pipeline" | "screening" } | null;

const SOURCE: Record<string, string> = {
  public: "website",
  portal: "portal",
  back_office: "back office",
};

export default function ApplicationsPage() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const allowed = can("application:read") || can("application:write");
  const canWrite = can("application:write");
  const canLease = can("lease:manage");
  const canScreen = can("screening:read");

  const apps = useApplications({ enabled: scoped && allowed });
  const properties = useProperties({ enabled: scoped && canLease });
  const catalog = useQuery({
    queryKey: ["applications", "workflow-catalog"],
    queryFn: () => api.applicationWorkflowCatalog(),
    enabled: scoped && allowed,
    staleTime: Infinity,
  });
  // Reuse only shows when the workspace turns it on.
  const settings = useSettings({ enabled: scoped && canWrite, retry: false });
  const reuseEnabled = Boolean(
    settings.data?.find((s) => s.key === "application_reuse.enabled")?.value
  );

  const { busy, run } = useRun([["applications"]]);
  const [stage, setStage] = useState("all");
  const [q, setQ] = useState("");
  const [open, setOpen] = useState<Open>(null);
  const [intaking, setIntaking] = useState(false);
  const [converting, setConverting] = useState<Application | null>(null);

  const all = useMemo(() => apps.data ?? [], [apps.data]);
  const counts = useMemo(() => {
    const m = new Map<string, number>();
    for (const a of all) m.set(a.status, (m.get(a.status) ?? 0) + 1);
    return m;
  }, [all]);
  // Stage chips in pipeline order, then any status the catalog doesn't name.
  const stages = useMemo(() => {
    const order = [
      ...(catalog.data?.stages ?? []),
      ...(catalog.data?.offramps ?? []),
    ].map((s) => s.key);
    const extra = [...counts.keys()].filter((k) => !order.includes(k));
    return [...order, ...extra].filter((k) => (counts.get(k) ?? 0) > 0);
  }, [catalog.data, counts]);
  const rows = useMemo(() => {
    const needle = q.trim().toLowerCase();
    return all
      .filter((a) => stage === "all" || a.status === stage)
      .filter(
        (a) =>
          !needle ||
          a.applicant_name.toLowerCase().includes(needle) ||
          a.email.toLowerCase().includes(needle)
      );
  }, [all, stage, q]);

  function transitionsFor(status: string): string[] {
    const list = [
      ...(catalog.data?.stages ?? []),
      ...(catalog.data?.offramps ?? []),
    ];
    return list.find((s) => s.key === status)?.transitions ?? [];
  }

  function toggle(id: string, part: "pipeline" | "screening") {
    setOpen((o) => (o?.id === id && o.part === part ? null : { id, part }));
  }

  if (!allowed) return <NoAccess what="applications" perm="application:read" />;

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Leasing"
        title="Applications"
        description="Every application, from the website, the renter portal or the office, in one pipeline: screening, approved, leased."
        actions={
          canWrite && (
            <Button onClick={() => setIntaking(true)}>
              <Plus />
              New application
            </Button>
          )
        }
      />

      {apps.data && all.length > 0 && (
        <div className="flex flex-wrap items-center gap-2">
          <Chip on={stage === "all"} onClick={() => setStage("all")}>
            All · {all.length}
          </Chip>
          {stages.map((k) => (
            <Chip key={k} on={stage === k} onClick={() => setStage(k)}>
              {k} · {counts.get(k)}
            </Chip>
          ))}
          <div className="relative w-full sm:ml-auto sm:w-64">
            <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-fg-3" />
            <Input
              value={q}
              onChange={(e) => setQ(e.target.value)}
              placeholder="Search name or email"
              aria-label="Search applications"
              className="h-10 pl-9"
            />
          </div>
        </div>
      )}

      {apps.error && (
        <Panel className="border-bad/30 p-4 text-[13px] text-bad">
          Couldn&apos;t load applications: {apps.error.message}
        </Panel>
      )}

      {apps.isLoading && (
        <div className="space-y-2">
          {Array.from({ length: 5 }, (_, i) => (
            <Skeleton key={i} className="h-16 rounded-2xl" />
          ))}
        </div>
      )}

      {apps.data && all.length === 0 && (
        <Panel>
          <EmptyState
            icon={<ClipboardList />}
            title="No applications yet"
            description="They arrive from your website and the renter portal, or enter one taken in person."
            action={
              canWrite && (
                <Button onClick={() => setIntaking(true)}>
                  <Plus />
                  New application
                </Button>
              )
            }
          />
        </Panel>
      )}

      {apps.data && all.length > 0 && rows.length === 0 && (
        <Panel>
          <EmptyState icon={<Search />} title="Nothing matches" />
        </Panel>
      )}

      {rows.length > 0 && (
        <Panel className="overflow-hidden">
          <ul className="divide-y divide-line">
            {rows.map((a, i) => {
              const nexts = transitionsFor(a.status).filter(
                (t) => t !== "Leased"
              );
              const canReuse =
                reuseEnabled &&
                canWrite &&
                a.status !== "Declined" &&
                a.status !== "Withdrawn";
              const part = open?.id === a.id ? open.part : null;
              return (
                <motion.li
                  key={a.id}
                  initial={{ opacity: 0, y: 6 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ delay: Math.min(i, 15) * 0.02, duration: 0.3 }}
                  className="px-4 py-3"
                >
                  <div className="flex flex-wrap items-center gap-3">
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="text-[14px] font-medium text-fg">
                          {a.applicant_name}
                        </span>
                        {a.has_pet && <Badge tone="warn">pet</Badge>}
                        {a.is_military && <Badge tone="info">military</Badge>}
                        {a.source !== "public" && (
                          <Badge tone="neutral">
                            {SOURCE[a.source] ?? a.source}
                          </Badge>
                        )}
                      </div>
                      <div className="truncate text-xs text-fg-3">
                        {a.email}
                        {a.credit_score ? ` · credit ${a.credit_score}` : ""}
                        {` · ${a.annual_income_label} a year`}
                        {a.move_in ? ` · moving ${a.move_in}` : ""}
                      </div>
                    </div>
                    {a.screening_status && (
                      <Badge
                        tone={a.screening_status === "cleared" ? "good" : "bad"}
                        className="hidden sm:inline-flex"
                      >
                        screening {a.screening_status}
                      </Badge>
                    )}
                    <Badge tone={statusTone(a.status)}>{a.status}</Badge>
                  </div>

                  <div className="mt-2 flex flex-wrap items-center gap-2">
                    <Toggle
                      on={part === "pipeline"}
                      onClick={() => toggle(a.id, "pipeline")}
                    >
                      Pipeline
                    </Toggle>
                    {canScreen && a.screening_consent_at && (
                      <Toggle
                        on={part === "screening"}
                        onClick={() => toggle(a.id, "screening")}
                      >
                        Screening report
                      </Toggle>
                    )}
                    <span className="flex-1" />
                    {canWrite &&
                      nexts.map((t) => (
                        <Button
                          key={t}
                          size="sm"
                          variant={
                            t === "Declined" || t === "Withdrawn"
                              ? "ghost"
                              : "secondary"
                          }
                          disabled={busy === a.id}
                          onClick={() =>
                            run(
                              a.id,
                              () => api.advanceApplication(a.id, t),
                              `Moved to ${t}`
                            )
                          }
                        >
                          {t}
                        </Button>
                      ))}
                    {canReuse && (
                      <Button
                        size="sm"
                        variant="ghost"
                        title="Copy this application so it can be used for another property"
                        disabled={busy === a.id}
                        onClick={() =>
                          run(
                            a.id,
                            () => api.reuseApplication(a.id),
                            "Application copied"
                          )
                        }
                      >
                        <Copy />
                        Reuse
                      </Button>
                    )}
                    {canLease && canWrite && a.status === "Approved" && (
                      <Button size="sm" onClick={() => setConverting(a)}>
                        <FilePlus2 />
                        Create lease
                      </Button>
                    )}
                  </div>

                  {part && (
                    <div className="mt-3 rounded-xl border border-line bg-fill/40 p-3">
                      {part === "pipeline" ? (
                        <Pipeline applicationId={a.id} />
                      ) : (
                        <Screening app={a} canWrite={canWrite} />
                      )}
                    </div>
                  )}
                </motion.li>
              );
            })}
          </ul>
        </Panel>
      )}

      <IntakeDialog open={intaking} onOpenChange={setIntaking} />
      <ConvertDialog
        app={converting}
        properties={properties.data ?? []}
        onOpenChange={(o) => !o && setConverting(null)}
      />
    </div>
  );
}

function Chip({
  on,
  onClick,
  children,
}: {
  on: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={on}
      className={cn(
        "rounded-full border px-3 py-1 text-[13px] transition",
        on
          ? "border-accent bg-accent/10 text-accent"
          : "border-line text-fg-3 hover:text-fg"
      )}
    >
      {children}
    </button>
  );
}

function Toggle({
  on,
  onClick,
  children,
}: {
  on: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      aria-expanded={on}
      onClick={onClick}
      className={cn(
        "inline-flex items-center gap-1 rounded-lg px-2 py-1 text-xs font-medium transition",
        on ? "bg-fill-2 text-fg" : "text-fg-3 hover:bg-fill-2 hover:text-fg"
      )}
    >
      {children}
      <ChevronDown className={cn("size-3.5 transition", on && "rotate-180")} />
    </button>
  );
}
