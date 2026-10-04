"use client";

// What opens under an application row: its pipeline (stages reached and the
// history of moves), and its screening report with the FCRA adverse-action
// notice when the application was declined on the report.

import { useQuery } from "@tanstack/react-query";
import { Send } from "lucide-react";
import { api } from "@/lib/api";
import type { Application } from "@/lib/types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/misc";
import { cn } from "@/lib/utils";
import { useRun } from "../leases/_ui/shared";

export function Pipeline({ applicationId }: { applicationId: string }) {
  const wf = useQuery({
    queryKey: ["applications", applicationId, "workflow"],
    queryFn: () => api.applicationWorkflow(applicationId),
  });

  if (wf.isLoading) return <Skeleton className="h-16" />;
  if (wf.error)
    return <p className="text-[13px] text-bad">{wf.error.message}</p>;
  const w = wf.data;
  if (!w) return null;

  return (
    <div className="space-y-3">
      <ol className="flex flex-wrap items-center gap-2">
        {w.stages.map((s, i) => (
          <li key={s.key} className="flex items-center gap-2">
            <span
              className={cn(
                "flex h-6 items-center rounded-full border px-2.5 text-xs font-medium",
                s.current
                  ? "border-accent bg-accent text-accent-fg"
                  : s.reached
                    ? "border-good/25 bg-good/12 text-good"
                    : "border-line bg-fill text-fg-3"
              )}
            >
              {s.label}
            </span>
            {i < w.stages.length - 1 && (
              <span className="h-px w-4 bg-line" aria-hidden />
            )}
          </li>
        ))}
        {w.offramps
          .filter((s) => s.current)
          .map((s) => (
            <li key={s.key}>
              <Badge tone="bad">{s.label}</Badge>
            </li>
          ))}
      </ol>

      {w.history.length > 0 && (
        <ul className="space-y-1 text-[13px] text-fg-2">
          {w.history.map((e) => (
            <li key={e.id}>
              <span className="figure text-xs text-fg-3">
                {e.created_at.slice(0, 10)}
              </span>{" "}
              {e.from_status ? `${e.from_status} to ` : ""}
              <span className="font-medium text-fg">{e.to_status}</span>
              {e.note ? `: ${e.note}` : ""}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

export function Screening({
  app,
  canWrite,
}: {
  app: Application;
  canWrite: boolean;
}) {
  const report = useQuery({
    queryKey: ["applications", app.id, "screening"],
    queryFn: () => api.screeningReport(app.id),
    retry: false,
  });
  const { busy, run } = useRun([["applications"]]);

  if (report.isLoading) return <Skeleton className="h-20" />;
  if (report.error)
    return <p className="text-[13px] text-fg-3">{report.error.message}</p>;
  const r = report.data;
  if (!r) return null;

  const adverse =
    r.result === "failed" ||
    (r.criminal_records ?? 0) > 0 ||
    (r.eviction_records ?? 0) > 0;

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-[13px] font-medium text-fg">
          Screening report from {r.provider}
        </span>
        <Badge tone={r.status === "complete" ? "good" : "neutral"}>
          {r.status.replace("_", " ")}
        </Badge>
        {r.result && (
          <Badge tone={r.result === "cleared" ? "good" : "bad"}>
            {r.result}
          </Badge>
        )}
        {r.consent_at && (
          <span className="text-xs text-fg-3">
            consent given {r.consent_at.slice(0, 10)}
          </span>
        )}
      </div>

      <dl className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        {[
          ["Credit score", r.credit_score],
          ["Criminal records", r.criminal_records],
          ["Eviction records", r.eviction_records],
          ["Provider says", r.recommendation],
        ].map(([label, value]) => (
          <div
            key={label}
            className="rounded-xl border border-line bg-surface p-3"
          >
            <dt className="eyebrow">{label}</dt>
            <dd className="figure mt-1 text-[17px] font-semibold text-fg">
              {value ?? "—"}
            </dd>
          </div>
        ))}
      </dl>

      {r.reasons.length > 0 && (
        <ul className="list-disc space-y-1 pl-5 text-[13px] text-fg-2">
          {r.reasons.map((x) => (
            <li key={x}>{x}</li>
          ))}
        </ul>
      )}

      {app.adverse_action_at ? (
        <p className="text-[13px] text-fg-3">
          Adverse-action notice sent {app.adverse_action_at.slice(0, 10)} and
          filed with the application&apos;s documents.
        </p>
      ) : (
        canWrite &&
        app.status === "Declined" &&
        adverse && (
          <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
            <Button
              size="sm"
              loading={busy === "notice"}
              onClick={() =>
                run(
                  "notice",
                  () => api.sendAdverseAction(app.id),
                  "Adverse-action notice sent"
                )
              }
            >
              <Send />
              Send adverse-action notice
            </Button>
            <span className="text-xs text-fg-3">
              FCRA §615(a). Names the reporting agency and the applicant&apos;s
              right to dispute. A PDF copy files with the application.
            </span>
          </div>
        )
      )}
    </div>
  );
}
