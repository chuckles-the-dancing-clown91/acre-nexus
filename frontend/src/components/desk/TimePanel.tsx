"use client";

// Hours on a work order: who worked it and for how long, a clock to start and
// stop, and a way to log time after the fact. Can be switched off per work
// order for jobs the in-house crew doesn't track.

import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Play, Plus, Square, Timer } from "lucide-react";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { me } from "@/lib/backoffice";
import { desk, minutesLabel } from "@/lib/servicedesk";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Panel, PanelHeader } from "@/components/ui/panel";

export function TimePanel({
  ticketId,
  tracking,
  manage,
  onLog,
  onChange,
}: {
  ticketId: string;
  tracking: boolean;
  manage: boolean;
  /** Open the log-time wizard. */
  onLog: () => void;
  onChange: () => void;
}) {
  const qc = useQueryClient();
  const q = useQuery({
    queryKey: ["time", ticketId],
    queryFn: () => desk.time(ticketId),
    refetchInterval: (query) =>
      query.state.data?.entries.some((e) => e.running) ? 30_000 : false,
  });
  const [busy, setBusy] = useState(false);
  const d = q.data;

  async function run(fn: () => Promise<unknown>, ok: string) {
    setBusy(true);
    try {
      await fn();
      toast.success(ok);
      await qc.invalidateQueries({ queryKey: ["time", ticketId] });
      onChange();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "That didn't work");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Panel>
      <PanelHeader
        title="Time"
        description={
          !tracking
            ? "Time tracking is off for this work order."
            : d
              ? `${minutesLabel(d.total_minutes) || "0m"} logged`
              : undefined
        }
        action={
          manage && (
            <label className="flex items-center gap-2 text-xs text-fg-3">
              Track time
              <input
                type="checkbox"
                className="size-4 accent-[var(--accent)]"
                checked={tracking}
                disabled={busy}
                onChange={(e) =>
                  void run(
                    () =>
                      api.updateTicket(ticketId, {
                        track_time: e.target.checked,
                      }),
                    e.target.checked
                      ? "Time tracking is on"
                      : "Time tracking is off"
                  )
                }
              />
            </label>
          )
        }
      />
      {tracking && (
        <div className="space-y-3 p-5 pt-2">
          {manage && (
            <div className="flex flex-wrap gap-2">
              {d?.my_running ? (
                <Button
                  size="sm"
                  variant="secondary"
                  loading={busy}
                  onClick={() =>
                    void run(() => me.clockOut({}), "Clock stopped")
                  }
                >
                  <Square />
                  Stop the clock
                </Button>
              ) : (
                <Button
                  size="sm"
                  loading={busy}
                  onClick={() =>
                    void run(
                      () =>
                        me.clockIn({
                          kind: "work_order",
                          maintenance_ticket_id: ticketId,
                        }),
                      "Clock started"
                    )
                  }
                >
                  <Play />
                  Start the clock
                </Button>
              )}
              <Button size="sm" variant="ghost" onClick={onLog}>
                <Plus />
                Log time
              </Button>
            </div>
          )}
          {d && d.entries.length === 0 && (
            <p className="flex items-center gap-2 text-[13px] text-fg-3">
              <Timer className="size-4" />
              No time yet.
            </p>
          )}
          <ul className="divide-y divide-line">
            {d?.entries.map((e) => (
              <li
                key={e.id}
                className="flex items-center gap-3 py-2 text-[13px]"
              >
                <div className="min-w-0 flex-1">
                  <div className="truncate font-medium text-fg">
                    {e.user_name}
                  </div>
                  <div className="truncate text-xs text-fg-3">
                    {new Date(e.started_at).toLocaleDateString(undefined, {
                      month: "short",
                      day: "numeric",
                    })}
                    {e.notes ? ` · ${e.notes}` : ""}
                  </div>
                </div>
                {e.running && <Badge tone="good">running</Badge>}
                <span className="figure text-fg">
                  {minutesLabel(e.minutes) || "0m"}
                </span>
              </li>
            ))}
          </ul>
        </div>
      )}
    </Panel>
  );
}
