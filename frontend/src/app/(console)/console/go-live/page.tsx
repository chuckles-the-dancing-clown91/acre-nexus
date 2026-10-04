"use client";

// Go live: is each provider ready to stop simulating, and is the deploy
// ready (production mode, public addresses, backups, a restore drill)?

import Link from "next/link";
import { useQuery } from "@tanstack/react-query";
import {
  CheckCircle2,
  ChevronRight,
  CircleAlert,
  CircleX,
  DatabaseBackup,
} from "lucide-react";
import {
  byUrgency,
  bytes,
  goLive,
  READINESS,
  type BackupRun,
} from "@/lib/golive";
import { useAuth } from "@/lib/auth";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge } from "@/components/ui/badge";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

function ago(iso: string | null): string {
  if (!iso) return "never";
  const mins = Math.round((Date.now() - new Date(iso).getTime()) / 60000);
  if (mins < 60) return `${Math.max(mins, 1)} min ago`;
  const h = Math.round(mins / 60);
  if (h < 48) return `${h} h ago`;
  return `${Math.round(h / 24)} days ago`;
}

function Mark({ ok }: { ok: boolean }) {
  return ok ? (
    <CheckCircle2 className="size-4 shrink-0 text-good" aria-label="Done" />
  ) : (
    <CircleX className="size-4 shrink-0 text-bad" aria-label="Not done" />
  );
}

export default function GoLivePage() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const allowed = can("integrations:manage");
  const q = useQuery({
    queryKey: ["go-live"],
    queryFn: goLive.get,
    enabled: scoped && allowed,
    refetchInterval: 60_000,
  });
  const d = q.data;
  const providers = [...(d?.providers ?? [])].sort(byUrgency);
  const live = providers.filter((p) => p.live);
  const problems = providers.filter((p) =>
    ["failing", "missing"].includes(p.readiness)
  );

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Admin"
        title="Go live"
        description="What's still simulated, what's missing, and whether the data is safe."
      />
      {!allowed && (
        <EmptyState
          title="Not for you"
          description="This page needs the integrations:manage permission."
        />
      )}
      {allowed && q.isLoading && <Skeleton className="h-64 rounded-2xl" />}
      {d && (
        <>
          <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
            <Tile
              label="Live providers"
              value={`${live.length} of ${providers.length}`}
            />
            <Tile
              label="Need attention"
              value={String(problems.length)}
              tone={problems.length ? "bad" : "good"}
            />
            <Tile
              label="Last good backup"
              value={ago(d.last_good_backup_at)}
              tone={
                d.platform.find((p) => p.key === "backup")?.ok ? "good" : "bad"
              }
            />
            <Tile
              label="Last restore drill"
              value={d.last_drill ? ago(d.last_drill.started_at) : "never"}
              tone={
                d.platform.find((p) => p.key === "drill")?.ok ? "good" : "bad"
              }
            />
          </div>

          <Panel>
            <PanelHeader
              title="Providers"
              description={
                d.live_providers
                  ? `LIVE_PROVIDERS=${d.live_providers}. Everything else answers with sample data.`
                  : "Nothing is live yet; every provider answers with sample data. Set LIVE_PROVIDERS on the server to switch one on."
              }
            />
            <ul className="divide-y divide-line">
              {providers.map((p) => {
                const r = READINESS[p.readiness];
                return (
                  <li key={p.key} className="px-5 py-4">
                    <div className="flex flex-wrap items-start gap-3">
                      <div className="min-w-0 flex-1">
                        <div className="flex flex-wrap items-center gap-2">
                          <span className="text-[14px] font-semibold text-fg">
                            {p.label}
                          </span>
                          <Badge tone={r.tone}>{r.word}</Badge>
                          {p.failures_7d > 0 && (
                            <Badge tone="bad">
                              {p.failures_7d} failed this week
                            </Badge>
                          )}
                        </div>
                        <p className="mt-0.5 text-[13px] text-fg-3">{p.what}</p>
                      </div>
                      <Link
                        href={p.href}
                        className="flex items-center gap-1 text-[13px] font-medium text-accent hover:underline"
                      >
                        Set up <ChevronRight className="size-3.5" />
                      </Link>
                    </div>
                    <div className="mt-3 grid gap-x-6 gap-y-1.5 text-[13px] sm:grid-cols-2">
                      <div className="flex items-center gap-2 text-fg-2">
                        <Mark ok={p.live} />
                        {p.live
                          ? "Switched on (LIVE_PROVIDERS)"
                          : `Not in LIVE_PROVIDERS (add "${p.key}")`}
                      </div>
                      {p.requirements.map((req) => (
                        <div
                          key={req.label}
                          className="flex items-center gap-2 text-fg-2"
                        >
                          <Mark ok={req.present} />
                          <span className="truncate font-mono text-[12px]">
                            {req.label}
                          </span>
                        </div>
                      ))}
                      <div className="flex items-center gap-2 text-fg-2">
                        {p.last_call_ok === null ? (
                          <CircleAlert
                            className="size-4 shrink-0 text-fg-4"
                            aria-label="No calls"
                          />
                        ) : (
                          <Mark ok={p.last_call_ok} />
                        )}
                        {p.last_call_at
                          ? `Last call ${ago(p.last_call_at)}${p.last_call_ok ? "" : p.last_error ? `: ${p.last_error}` : ", failed"}`
                          : p.live
                            ? "No real calls in the last 30 days"
                            : "No real calls yet (sample data only)"}
                      </div>
                      {p.webhook_expected && (
                        <div className="flex items-center gap-2 text-fg-2">
                          {p.last_webhook_at ? (
                            <Mark ok />
                          ) : (
                            <CircleAlert
                              className="size-4 shrink-0 text-fg-4"
                              aria-label="None yet"
                            />
                          )}
                          {p.last_webhook_at
                            ? `Signed webhook ${ago(p.last_webhook_at)}`
                            : "No signed webhook yet"}
                        </div>
                      )}
                    </div>
                  </li>
                );
              })}
            </ul>
          </Panel>

          <div className="grid gap-6 lg:grid-cols-2">
            <Panel>
              <PanelHeader
                title="The deploy"
                description="Checked on the server; these aren't settings you can change here."
              />
              <ul className="space-y-3 px-5 pb-5">
                {d.platform.map((c) => (
                  <li key={c.key} className="flex gap-2.5">
                    <Mark ok={c.ok} />
                    <div className="min-w-0">
                      <div className="text-[13px] font-medium text-fg">
                        {c.label}
                      </div>
                      <div className="text-[12px] break-words text-fg-3">
                        {c.detail}
                      </div>
                    </div>
                  </li>
                ))}
              </ul>
            </Panel>
            <Panel>
              <PanelHeader
                title="Backups"
                description="Recorded by backend/deploy/backup.sh and restore-drill.sh."
              />
              <div className="space-y-3 px-5 pb-5">
                <Run title="Newest backup" run={d.last_backup} />
                <Run title="Newest restore drill" run={d.last_drill} />
              </div>
            </Panel>
          </div>
        </>
      )}
    </div>
  );
}

function Tile({
  label,
  value,
  tone,
}: {
  label: string;
  value: string;
  tone?: "good" | "bad";
}) {
  return (
    <div className="glass rounded-2xl p-4">
      <div className="eyebrow">{label}</div>
      <div
        className={cn(
          "figure mt-1.5 text-[22px] leading-none font-semibold",
          tone === "good"
            ? "text-good"
            : tone === "bad"
              ? "text-bad"
              : "text-fg"
        )}
      >
        {value}
      </div>
    </div>
  );
}

function Run({ title, run }: { title: string; run: BackupRun | null }) {
  return (
    <div className="flex gap-3 rounded-xl border border-line p-3">
      <DatabaseBackup
        className={cn(
          "mt-0.5 size-4 shrink-0",
          run ? (run.ok ? "text-good" : "text-bad") : "text-fg-4"
        )}
      />
      <div className="min-w-0 text-[13px]">
        <div className="font-medium text-fg">
          {title} ·{" "}
          {run
            ? `${run.ok ? "worked" : "failed"} ${ago(run.started_at)}`
            : "none recorded"}
        </div>
        {run && (
          <div className="mt-0.5 text-[12px] break-all text-fg-3">
            {[run.detail, bytes(run.bytes), run.location]
              .filter(Boolean)
              .join(" · ")}
          </div>
        )}
      </div>
    </div>
  );
}
