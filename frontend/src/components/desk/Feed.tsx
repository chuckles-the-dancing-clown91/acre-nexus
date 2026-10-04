"use client";

// The work order's feed: what happened, newest first. Notes, status moves,
// photos and video, expenses, time, finished tasks and what vendors said,
// all in one stream. Pass a task and it shows just that task's.

import { useQuery } from "@tanstack/react-query";
import {
  ArrowRightLeft,
  Camera,
  CheckCircle2,
  Clock,
  FileText,
  HardHat,
  Home,
  Lock,
  MessageSquare,
  Receipt,
  Zap,
} from "lucide-react";
import { desk, minutesLabel, money, type FeedItem } from "@/lib/servicedesk";
import { Badge } from "@/components/ui/badge";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Media } from "@/components/desk/Notes";
import { cn } from "@/lib/utils";

const ICON: Record<FeedItem["kind"], typeof Zap> = {
  note: MessageSquare,
  resident: Home,
  status: ArrowRightLeft,
  action: Zap,
  photo: Camera,
  video: Camera,
  file: FileText,
  expense: Receipt,
  time: Clock,
  task: CheckCircle2,
  vendor: HardHat,
};

const TONE: Record<FeedItem["kind"], string> = {
  note: "bg-accent/12 text-accent",
  resident: "bg-info/15 text-info",
  status: "bg-fill-2 text-fg-2",
  action: "bg-warn/15 text-warn",
  photo: "bg-fill-2 text-fg-2",
  video: "bg-fill-2 text-fg-2",
  file: "bg-fill-2 text-fg-2",
  expense: "bg-good/15 text-good",
  time: "bg-accent/12 text-accent",
  task: "bg-good/15 text-good",
  vendor: "bg-warn/15 text-warn",
};

function day(iso: string): string {
  const d = new Date(iso);
  const today = new Date();
  const same = (a: Date, b: Date) => a.toDateString() === b.toDateString();
  const yesterday = new Date(today.getTime() - 86_400_000);
  if (same(d, today)) return "Today";
  if (same(d, yesterday)) return "Yesterday";
  return d.toLocaleDateString(undefined, {
    weekday: "long",
    month: "long",
    day: "numeric",
  });
}

function clock(iso: string): string {
  return new Date(iso).toLocaleTimeString(undefined, {
    hour: "numeric",
    minute: "2-digit",
  });
}

export function Feed({
  ticketId,
  taskId,
  refreshKey,
  empty = "Nothing yet. Add a note, a photo or an update.",
  onOpenTask,
}: {
  ticketId: string;
  taskId?: string;
  /** Changes when something was added, so the feed reloads. */
  refreshKey?: number;
  empty?: string;
  onOpenTask?: (taskId: string) => void;
}) {
  const q = useQuery({
    queryKey: ["feed", ticketId, taskId ?? "all", refreshKey ?? 0],
    queryFn: () => desk.feed(ticketId, taskId),
  });
  if (q.isLoading) return <Skeleton className="h-40 rounded-2xl" />;
  const items = q.data ?? [];
  if (items.length === 0)
    return (
      <EmptyState icon={<MessageSquare />} title={empty} className="py-10" />
    );

  const groups: { day: string; items: FeedItem[] }[] = [];
  for (const it of items) {
    const d = day(it.at);
    const last = groups[groups.length - 1];
    if (last?.day === d) last.items.push(it);
    else groups.push({ day: d, items: [it] });
  }

  return (
    <div className="space-y-5">
      {groups.map((g) => (
        <section key={g.day}>
          <div className="eyebrow mb-2">{g.day}</div>
          <ol className="relative space-y-3 before:absolute before:top-2 before:bottom-2 before:left-[15px] before:w-px before:bg-line">
            {g.items.map((it) => {
              const Icon = ICON[it.kind] ?? Zap;
              const quiet = it.kind === "status" || it.kind === "action";
              return (
                <li key={it.id} className="relative flex gap-3">
                  <span
                    className={cn(
                      "z-10 flex size-8 shrink-0 items-center justify-center rounded-full ring-4 ring-bg",
                      TONE[it.kind]
                    )}
                  >
                    <Icon className="size-4" />
                  </span>
                  <div className="min-w-0 flex-1 pt-0.5">
                    <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-fg-3">
                      <span className="font-medium text-fg-2">
                        {it.actor ?? "System"}
                      </span>
                      <span>{clock(it.at)}</span>
                      {it.kind === "resident" && (
                        <Badge tone="info">Resident</Badge>
                      )}
                      {it.visibility === "internal" && it.kind === "note" && (
                        <Badge>
                          <Lock className="size-3" />
                          Staff
                        </Badge>
                      )}
                      {it.task_id && it.task_title && !onOpenTask && (
                        <span className="text-fg-4">on {it.task_title}</span>
                      )}
                      {it.task_id && it.task_title && onOpenTask && (
                        <button
                          type="button"
                          onClick={() => onOpenTask(it.task_id!)}
                          className="text-accent hover:underline"
                        >
                          on {it.task_title}
                        </button>
                      )}
                    </div>
                    {it.title && (
                      <p
                        className={cn(
                          "mt-0.5 text-[13px]",
                          quiet ? "text-fg-2" : "font-medium text-fg"
                        )}
                      >
                        {it.title}
                        {it.kind === "expense" && it.amount_cents != null && (
                          <span className="figure ml-2 text-fg">
                            {money(it.amount_cents)}
                          </span>
                        )}
                        {it.kind === "time" && it.minutes != null && (
                          <span className="figure ml-2 text-fg">
                            {it.running ? "running · " : ""}
                            {minutesLabel(it.minutes)}
                          </span>
                        )}
                      </p>
                    )}
                    {it.body && (
                      <p className="mt-0.5 text-[13px] whitespace-pre-wrap text-fg">
                        {it.body}
                      </p>
                    )}
                    {it.files.length > 0 && (
                      <div className="mt-2 flex flex-wrap gap-2">
                        {it.files.map((f) => (
                          <Media key={f.id} file={f} className="size-24" />
                        ))}
                      </div>
                    )}
                  </div>
                </li>
              );
            })}
          </ol>
        </section>
      ))}
    </div>
  );
}
