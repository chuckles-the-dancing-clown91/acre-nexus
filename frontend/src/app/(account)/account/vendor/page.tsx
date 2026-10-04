"use client";

// The vendor portal: every job sent to this vendor, open ones first.

import Link from "next/link";
import { useQuery } from "@tanstack/react-query";
import { ChevronRight, HardHat, MapPin } from "lucide-react";
import {
  vendorPortal,
  vendorResponseWords,
  type PortalJobRow,
} from "@/lib/vendorLink";
import { ApiError } from "@/lib/api";
import { Badge } from "@/components/ui/badge";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";

function when(iso: string | null) {
  return iso
    ? new Date(iso).toLocaleDateString(undefined, {
        month: "short",
        day: "numeric",
      })
    : "";
}

export default function VendorJobsPage() {
  const me = useQuery({
    queryKey: ["vendor-portal", "me"],
    queryFn: vendorPortal.me,
    retry: false,
  });
  const jobs = useQuery({
    queryKey: ["vendor-portal", "jobs"],
    queryFn: vendorPortal.jobs,
    enabled: !!me.data,
    refetchInterval: 60_000,
  });
  if (me.error) {
    return (
      <Panel>
        <EmptyState
          icon={<HardHat />}
          title="This login isn't set up as a vendor"
          description={
            me.error instanceof ApiError && me.error.status === 403
              ? "Ask the property manager to invite you to their vendor portal."
              : me.error.message
          }
        />
      </Panel>
    );
  }
  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-[24px] font-semibold text-fg">
          {me.data ? me.data.vendor : "Jobs"}
        </h1>
        <p className="text-[13px] text-fg-3">
          {me.data
            ? `Work from ${me.data.company}. ${me.data.open_jobs} waiting on you.`
            : "Loading…"}
        </p>
      </div>
      {jobs.isLoading && <Skeleton className="h-40" />}
      {jobs.data && (
        <>
          <Section
            title="Open"
            rows={jobs.data.open}
            empty="Nothing waiting on you."
          />
          {jobs.data.closed.length > 0 && (
            <Section title="Finished" rows={jobs.data.closed} empty="" />
          )}
        </>
      )}
    </div>
  );
}

function Section({
  title,
  rows,
  empty,
}: {
  title: string;
  rows: PortalJobRow[];
  empty: string;
}) {
  return (
    <section className="space-y-2">
      <div className="eyebrow">{title}</div>
      {rows.length === 0 ? (
        <Panel className="px-4 py-6 text-center text-[13px] text-fg-3">
          {empty}
        </Panel>
      ) : (
        <Panel className="divide-y divide-line overflow-hidden">
          {rows.map((j) => (
            <Link
              key={j.batch}
              href={`/account/vendor/${j.batch}`}
              className="flex items-center gap-3 px-4 py-3.5 hover:bg-surface-2"
            >
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="truncate text-[15px] font-semibold text-fg">
                    {j.title}
                  </span>
                  {j.priority === "high" || j.priority === "urgent" ? (
                    <Badge tone="bad">{j.priority}</Badge>
                  ) : null}
                  <Badge
                    tone={
                      j.response === "accepted"
                        ? "info"
                        : j.response === "done"
                          ? "good"
                          : "warn"
                    }
                  >
                    {(j.response &&
                      vendorResponseWords(
                        j.response as "accepted" | "declined" | "done"
                      )) ||
                      "needs an answer"}
                  </Badge>
                </div>
                <div className="mt-0.5 flex items-center gap-1 truncate text-[12px] text-fg-3">
                  <MapPin className="size-3" />
                  {j.property}
                </div>
                <div className="text-[12px] text-fg-4">
                  {j.tasks_done} of {j.tasks} task{j.tasks === 1 ? "" : "s"}{" "}
                  done
                  {j.sent_at && ` · sent ${when(j.sent_at)}`}
                  {j.due_date && ` · wanted by ${j.due_date}`}
                </div>
              </div>
              <ChevronRight className="size-4 shrink-0 text-fg-4" />
            </Link>
          ))}
        </Panel>
      )}
    </section>
  );
}
