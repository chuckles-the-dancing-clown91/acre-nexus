"use client";

// What needs a date: routines coming due within the lead time with no open
// work order, and open work orders with no due date and no visit booked.
// "Open now" opens the routine's work order early; a work order's row goes to
// its Visit panel.

import Link from "next/link";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CalendarClock, CalendarPlus, ShieldCheck } from "lucide-react";
import { toast } from "sonner";
import { useRouter } from "next/navigation";
import { attention, dueWords } from "@/lib/attention";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

export function ToSchedule({
  manage,
  names,
  enabled = true,
}: {
  manage: boolean;
  names: Map<string, string>;
  enabled?: boolean;
}) {
  const qc = useQueryClient();
  const router = useRouter();
  const q = useQuery({
    queryKey: ["to-schedule"],
    queryFn: attention.toSchedule,
    enabled,
  });
  const open = useMutation({
    mutationFn: (planId: string) => attention.runNow(planId),
    onSuccess: (t) => {
      toast.success("Work order opened");
      void qc.invalidateQueries({ queryKey: ["to-schedule"] });
      void qc.invalidateQueries({ queryKey: ["tickets"] });
      void qc.invalidateQueries({ queryKey: ["plans"] });
      router.push(`/console/maintenance/${t.id}`);
    },
    onError: (e) =>
      toast.error(e instanceof Error ? e.message : "Couldn't open it"),
  });

  const plans = q.data?.plans ?? [];
  const tickets = q.data?.tickets ?? [];
  if (!q.data || (plans.length === 0 && tickets.length === 0)) return null;

  return (
    <Panel className="overflow-hidden">
      <div className="flex items-center justify-between gap-3 border-b border-line px-4 py-3">
        <div>
          <div className="flex items-center gap-2 text-[14px] font-semibold text-fg">
            <CalendarClock className="size-4 text-warn" />
            To schedule
          </div>
          <div className="text-xs text-fg-3">
            Routines due within {q.data.lead_days} days and work orders with no
            date or visit.
          </div>
        </div>
        <Badge tone="warn">{plans.length + tickets.length}</Badge>
      </div>
      <ul className="divide-y divide-line">
        {plans.map((d) => (
          <li key={d.plan.id} className="flex items-center gap-3 px-4 py-3">
            <span
              className={cn(
                "size-2 shrink-0 rounded-full",
                d.days < 0 ? "bg-bad" : d.days <= 7 ? "bg-warn" : "bg-fg-4"
              )}
              aria-hidden
            />
            <div className="min-w-0 flex-1">
              <div className="flex flex-wrap items-center gap-2">
                <span className="truncate text-[14px] font-medium text-fg">
                  {d.plan.title}
                </span>
                {d.mandate && (
                  <Badge tone="info">
                    <ShieldCheck className="size-3" />
                    required by code
                  </Badge>
                )}
              </div>
              <div className="truncate text-xs text-fg-3">
                {d.property_name} · routine {dueWords(d.days)} ·{" "}
                {d.plan.next_due_date}
              </div>
            </div>
            {manage ? (
              <Button
                size="sm"
                variant="secondary"
                disabled={open.isPending}
                onClick={() => open.mutate(d.plan.id)}
              >
                <CalendarPlus />
                Open now
              </Button>
            ) : (
              <Link
                href="/console/maintenance/schedule"
                className="text-xs text-fg-3 hover:text-fg"
              >
                Schedule
              </Link>
            )}
          </li>
        ))}
        {tickets.map((t) => (
          <li key={t.id} className="flex items-center gap-3 px-4 py-3">
            <span
              className={cn(
                "size-2 shrink-0 rounded-full",
                t.priority === "urgent"
                  ? "bg-bad"
                  : t.priority === "high"
                    ? "bg-warn"
                    : "bg-fg-4"
              )}
              aria-hidden
            />
            <Link
              href={`/console/maintenance/${t.id}`}
              className="min-w-0 flex-1"
            >
              <div className="truncate text-[14px] font-medium text-fg">
                {t.title}
              </div>
              <div className="truncate text-xs text-fg-3">
                {names.get(t.property_id) ?? "Property"} · no date, no visit
                {t.assignee_name ? ` · ${t.assignee_name}` : " · unassigned"}
              </div>
            </Link>
            <Badge tone={t.priority === "urgent" ? "bad" : "neutral"}>
              {t.priority}
            </Badge>
            <Button size="sm" variant="secondary" asChild>
              <Link href={`/console/maintenance/${t.id}#visit`}>
                <CalendarPlus />
                Book
              </Link>
            </Button>
          </li>
        ))}
      </ul>
    </Panel>
  );
}
