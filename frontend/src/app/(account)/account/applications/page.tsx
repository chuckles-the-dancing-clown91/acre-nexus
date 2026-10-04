"use client";

// An applicant's applications: every one they sent, from the portal or the
// public site with the same email, and where each stands.

import Link from "next/link";
import { useQuery } from "@tanstack/react-query";
import { Check, ClipboardList } from "lucide-react";
import { api, ApiError } from "@/lib/api";
import type { Application } from "@/lib/types";
import {
  APPLICATION_STEPS,
  applicationNote,
  applicationTone,
  day,
} from "@/lib/portal-format";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

export default function ApplicationsPage() {
  const apps = useQuery({
    queryKey: ["my-applications"],
    queryFn: api.myApplications,
    retry: (n, e) => !(e instanceof ApiError && e.status < 500) && n < 2,
  });

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-[24px] font-semibold text-fg">Applications</h1>
        <p className="text-[13px] text-fg-3">
          Screening runs on its own and we email you at each step.
        </p>
        <Button asChild size="sm" className="mt-3">
          <Link href="/account/apply">Apply now</Link>
        </Button>
      </div>

      {apps.isLoading && <Skeleton className="h-40" />}

      {apps.error && (
        <Panel>
          <EmptyState
            icon={<ClipboardList />}
            title="Couldn't load your applications"
            description={apps.error.message}
          />
        </Panel>
      )}

      {apps.isSuccess && apps.data.length === 0 && (
        <Panel>
          <EmptyState
            icon={<ClipboardList />}
            title="No applications yet"
            description="Find a home you like and apply. It shows up here."
            action={
              <Button asChild>
                <Link href="/">Browse homes</Link>
              </Button>
            }
          />
        </Panel>
      )}

      {apps.data && apps.data.length > 0 && (
        <div className="space-y-3">
          {apps.data.map((a) => (
            <ApplicationCard key={a.id} a={a} />
          ))}
        </div>
      )}
    </div>
  );
}

function ApplicationCard({ a }: { a: Application }) {
  const offRamp = a.status === "Declined" || a.status === "Withdrawn";
  const at = APPLICATION_STEPS.findIndex((s) => s.key === a.status);
  const note = applicationNote(a.status);

  return (
    <Panel className="space-y-4 p-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="text-[15px] font-semibold text-fg">
            Sent {day(a.created_at)}
          </div>
          <div className="text-xs text-fg-3">
            {a.move_in ? `Move-in ${day(a.move_in)}` : a.applicant_name}
          </div>
        </div>
        <div className="flex flex-wrap gap-1.5">
          {a.screening_status && (
            <Badge tone={a.screening_status === "cleared" ? "good" : "bad"}>
              {a.screening_status === "cleared"
                ? "Screening cleared"
                : "Screening not cleared"}
            </Badge>
          )}
          <Badge tone={applicationTone(a.status)}>
            {APPLICATION_STEPS.find((s) => s.key === a.status)?.label ??
              a.status}
          </Badge>
        </div>
      </div>

      <ol className="flex items-center gap-1.5" aria-label="Progress">
        {APPLICATION_STEPS.map((s, i) => {
          const current = s.key === a.status;
          const reached = !offRamp && at >= 0 && i <= at;
          return (
            <li
              key={s.key}
              aria-current={current ? "step" : undefined}
              className="flex min-w-0 flex-1 flex-col gap-1.5"
            >
              <span
                className={cn(
                  "h-1.5 rounded-full",
                  current ? "bg-accent" : reached ? "bg-good" : "bg-fill-2"
                )}
              />
              <span
                className={cn(
                  "flex items-center gap-1 truncate text-[11px]",
                  current
                    ? "font-medium text-fg"
                    : reached
                      ? "text-fg-2"
                      : "text-fg-4"
                )}
              >
                {reached && !current && <Check className="size-3" />}
                {s.label}
              </span>
            </li>
          );
        })}
      </ol>

      {note && <p className="text-[13px] text-fg-2">{note}</p>}
    </Panel>
  );
}
