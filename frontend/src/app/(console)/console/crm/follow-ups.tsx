"use client";

// Follow-ups that are due (or, on request, every open one), across owners and
// leads. Done, snooze a day or a week, or open the timeline.

import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { BellRing, Check, ChevronRight, Clock } from "lucide-react";
import { toast } from "sonner";
import { crm, type CrmNote } from "@/lib/backoffice";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Tabs } from "@/components/ui/data-table";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { errMsg, snoozeDay, SUBJECT_LABEL, type Opened } from "./shared";
import { followUpState, KindBadge, prettyDay } from "./timeline";

const VIEWS = [
  ["due", "Due now"],
  ["all", "Include upcoming"],
] as const;

export function FollowUps({
  enabled,
  manage,
  onOpen,
}: {
  enabled: boolean;
  manage: boolean;
  onOpen: (o: Opened) => void;
}) {
  const qc = useQueryClient();
  const [view, setView] = useState<"due" | "all">("due");
  const items = useQuery({
    queryKey: ["crm", "follow-ups", view === "all"],
    queryFn: () => crm.followUps(view === "all"),
    enabled,
  });

  async function patch(
    n: CrmNote,
    body: Parameters<typeof crm.updateNote>[1],
    ok: string
  ) {
    try {
      await crm.updateNote(n.id, body);
      toast.success(ok);
      await qc.invalidateQueries({ queryKey: ["crm"] });
    } catch (e) {
      toast.error(errMsg(e, "Couldn't update the follow-up"));
    }
  }

  const open = (n: CrmNote) =>
    onOpen({ type: n.subject_type, id: n.subject_id, name: n.subject_name });

  return (
    <div className="space-y-4">
      <Tabs tabs={VIEWS} value={view} onChange={setView} />
      <Panel className="overflow-hidden">
        {items.isLoading && (
          <div className="space-y-2 p-3">
            {Array.from({ length: 4 }, (_, i) => (
              <Skeleton key={i} className="h-20" />
            ))}
          </div>
        )}
        {items.error && (
          <p className="p-4 text-[13px] text-bad">
            Couldn&apos;t load follow-ups: {items.error.message}
          </p>
        )}
        {items.data?.length === 0 && (
          <EmptyState
            icon={<BellRing />}
            title={view === "all" ? "No open follow-ups" : "All caught up"}
            description={
              view === "all"
                ? "Add one from an owner's or lead's timeline."
                : "Nothing is due today."
            }
          />
        )}
        <ul className="divide-y divide-line">
          {items.data?.map((n) => {
            const fu = n.follow_up_on ? followUpState(n.follow_up_on) : null;
            return (
              <li
                key={n.id}
                className="flex flex-col gap-3 px-5 py-4 sm:flex-row sm:items-start"
              >
                <div className="min-w-0 flex-1 space-y-1.5">
                  <div className="flex flex-wrap items-center gap-2">
                    <button
                      type="button"
                      onClick={() => open(n)}
                      className="truncate text-[14px] font-medium text-fg hover:text-accent"
                    >
                      {n.subject_name ?? "Unknown"}
                    </button>
                    <Badge>{SUBJECT_LABEL[n.subject_type]}</Badge>
                    <KindBadge kind={n.kind} />
                    {fu && <Badge tone={fu.tone}>{fu.label}</Badge>}
                  </div>
                  <p className="line-clamp-2 text-[13px] whitespace-pre-wrap text-fg-2">
                    {n.body}
                  </p>
                  <p className="text-xs text-fg-3">
                    {n.author ?? "System"} ·{" "}
                    {new Date(n.created_at).toLocaleDateString([], {
                      month: "short",
                      day: "numeric",
                    })}
                  </p>
                </div>
                <div className="flex flex-wrap items-center gap-1.5">
                  {manage && (
                    <>
                      <Button
                        size="sm"
                        variant="secondary"
                        onClick={() =>
                          patch(n, { follow_up_done: true }, "Marked done")
                        }
                      >
                        <Check />
                        Done
                      </Button>
                      {[
                        [1, "+1 day"],
                        [7, "+1 week"],
                      ].map(([days, label]) => (
                        <Button
                          key={label}
                          size="sm"
                          variant="ghost"
                          onClick={() => {
                            const d = snoozeDay(n.follow_up_on, days as number);
                            void patch(
                              n,
                              { follow_up_on: d },
                              `Snoozed to ${prettyDay(d)}`
                            );
                          }}
                        >
                          <Clock />
                          {label}
                        </Button>
                      ))}
                    </>
                  )}
                  <Button size="sm" variant="ghost" onClick={() => open(n)}>
                    Open
                    <ChevronRight />
                  </Button>
                </div>
              </li>
            );
          })}
        </ul>
      </Panel>
    </div>
  );
}
